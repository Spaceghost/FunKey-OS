use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use iroh::{EndpointId, endpoint::Connection};
use tokio::{
    fs::{self, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
    signal,
    time::timeout,
};

use crate::{
    bundle,
    config::{Paths, max_save_bytes, online_timeout},
    endpoint,
    identity,
    peers::PeerBook,
    save::{hash_file, sanitize_component},
    wire::{
        BUNDLE_ALPN, SAVE_ALPN, read_header, write_status,
    },
};

const COPY_BUFFER_BYTES: usize = 64 * 1024;
const CLOSE_GRACE: Duration = Duration::from_secs(5);

pub async fn serve(paths: Paths, allow_unpaired: bool) -> Result<()> {
    paths.ensure()?;
    let secret = identity::load_or_create(&paths.identity)?;
    let endpoint = endpoint::bind(
        secret,
        vec![SAVE_ALPN.to_vec(), BUNDLE_ALPN.to_vec()],
    )
    .await?;
    let ticket = endpoint::ticket(
        &endpoint,
        &paths.current_ticket,
        online_timeout()?,
    )
    .await?;

    println!("{ticket}");
    eprintln!(
        "funkey-iroh: progress and portable-bundle receiver online as {} (paired_only={})",
        endpoint.id(),
        !allow_unpaired
    );

    let shutdown = signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            incoming = endpoint.accept() => {
                let Some(incoming) = incoming else {
                    break;
                };
                let connection = match incoming.await {
                    Ok(connection) => connection,
                    Err(error) => {
                        eprintln!("funkey-iroh: rejected incoming connection: {error:#}");
                        continue;
                    }
                };
                let connection_paths = paths.clone();
                tokio::spawn(async move {
                    let protocol = connection.alpn().to_vec();
                    let result = if protocol.as_slice() == SAVE_ALPN {
                        handle_save_connection(
                            connection,
                            connection_paths,
                            allow_unpaired,
                        )
                        .await
                    } else if protocol.as_slice() == BUNDLE_ALPN {
                        bundle::handle_bundle_connection(
                            connection,
                            connection_paths,
                            allow_unpaired,
                        )
                        .await
                    } else {
                        connection.close(2u32.into(), b"unsupported protocol");
                        Ok(())
                    };
                    if let Err(error) = result {
                        eprintln!("funkey-iroh: incoming transfer failed: {error:#}");
                    }
                });
            }
            result = &mut shutdown => {
                result.context("wait for shutdown signal")?;
                break;
            }
        }
    }

    endpoint.close().await;
    Ok(())
}

async fn handle_save_connection(
    connection: Connection,
    paths: Paths,
    allow_unpaired: bool,
) -> Result<()> {
    let remote_id = connection.remote_id();
    let peer_book = PeerBook::new(paths.peers.clone());
    let paired_name = peer_book.name_for(&remote_id)?;

    if paired_name.is_none() && !allow_unpaired {
        eprintln!("funkey-iroh: refusing unpaired save sender {remote_id}");
        connection.close(1u32.into(), b"unpaired endpoint");
        return Ok(());
    }

    let peer_directory = paired_name
        .as_deref()
        .map(sanitize_component)
        .unwrap_or_else(|| short_endpoint_id(&remote_id));

    receive_save(connection, &paths, &peer_directory).await
}

async fn receive_save(
    connection: Connection,
    paths: &Paths,
    peer_directory: &str,
) -> Result<()> {
    let remote_id = connection.remote_id();
    let (mut send, mut recv) = connection
        .accept_bi()
        .await
        .context("accept save transfer stream")?;

    let header = match read_header(&mut recv).await {
        Ok(header) => header,
        Err(error) => {
            let _ = write_status(
                &mut send,
                "ERROR",
                &format!("invalid header: {error:#}"),
            )
            .await;
            let _ = send.finish();
            return Err(error).context("read save transfer header");
        }
    };

    let maximum = max_save_bytes()?;
    if header.size > maximum {
        write_status(
            &mut send,
            "ERROR",
            &format!(
                "save is {} bytes; receiver limit is {maximum}",
                header.size
            ),
        )
        .await?;
        send.finish().context("finish oversized-save response")?;
        return Ok(());
    }

    let system = sanitize_component(&header.system);
    let game = sanitize_component(&header.game);
    let filename = sanitize_component(&header.filename);
    let destination_dir = paths
        .inbox
        .join(peer_directory)
        .join(system)
        .join(game);
    fs::create_dir_all(&destination_dir)
        .await
        .with_context(|| format!("create save destination {}", destination_dir.display()))?;

    let requested_destination = destination_dir.join(filename);
    if requested_destination.exists() {
        let (existing_hash, existing_size) = hash_file(&requested_destination).await?;
        if existing_size == header.size && existing_hash == header.hash {
            write_status(
                &mut send,
                "SKIP",
                &requested_destination.display().to_string(),
            )
            .await?;
            send.finish().context("finish duplicate-save response")?;
            let _ = timeout(CLOSE_GRACE, connection.closed()).await;
            return Ok(());
        }
    }

    let conflict = requested_destination.exists();
    let destination = if conflict {
        conflict_path(&requested_destination, &header.hash)
    } else {
        requested_destination
    };
    let temporary = temporary_path(&destination);

    write_status(&mut send, "READY", "").await?;

    let receive_result =
        receive_file(&mut recv, &temporary, header.size, &header.hash).await;
    if let Err(error) = receive_result {
        let _ = fs::remove_file(&temporary).await;
        let _ = write_status(&mut send, "ERROR", &format!("{error:#}")).await;
        let _ = send.finish();
        return Err(error);
    }

    fs::rename(&temporary, &destination).await.with_context(|| {
        format!(
            "atomically install received save {} as {}",
            temporary.display(),
            destination.display()
        )
    })?;

    let code = if conflict { "CONFLICT" } else { "STORED" };
    write_status(&mut send, code, &destination.display().to_string()).await?;
    send.finish().context("finish final save response")?;

    eprintln!(
        "funkey-iroh: received {} bytes from {remote_id} into {}",
        header.size,
        destination.display()
    );
    let _ = timeout(CLOSE_GRACE, connection.closed()).await;
    Ok(())
}

async fn receive_file<R>(
    reader: &mut R,
    temporary: &Path,
    size: u64,
    expected_hash: &[u8; 32],
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)
        .await
        .with_context(|| format!("create incoming save {}", temporary.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut remaining = size;
    let mut buffer = vec![0u8; COPY_BUFFER_BYTES];

    while remaining > 0 {
        let wanted = usize::try_from(remaining.min(buffer.len() as u64))
            .expect("bounded by the fixed buffer length");
        let read = reader
            .read(&mut buffer[..wanted])
            .await
            .context("read incoming save contents")?;
        if read == 0 {
            bail!("incoming save ended with {remaining} bytes missing");
        }
        file.write_all(&buffer[..read])
            .await
            .with_context(|| format!("write incoming save {}", temporary.display()))?;
        hasher.update(&buffer[..read]);
        remaining -= read as u64;
    }

    file.sync_all()
        .await
        .with_context(|| format!("sync incoming save {}", temporary.display()))?;
    drop(file);

    let actual_hash = *hasher.finalize().as_bytes();
    if &actual_hash != expected_hash {
        bail!(
            "BLAKE3 mismatch: expected {}, received {}",
            hex(expected_hash),
            hex(&actual_hash)
        );
    }
    Ok(())
}

fn short_endpoint_id(endpoint_id: &EndpointId) -> String {
    endpoint_id.to_string().chars().take(16).collect()
}

fn temporary_path(destination: &Path) -> PathBuf {
    let sequence = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let filename = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("save");
    destination.with_file_name(format!(
        ".{filename}.part-{}-{sequence}",
        std::process::id()
    ))
}

fn conflict_path(destination: &Path, hash: &[u8; 32]) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let hash_prefix = &hex(hash)[..12];
    let stem = destination
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("save");
    let extension = destination
        .extension()
        .and_then(|extension| extension.to_str());

    let filename = match extension {
        Some(extension) if !extension.is_empty() => {
            format!(
                "{stem}.conflict-{timestamp}-{}-{hash_prefix}.{extension}",
                std::process::id()
            )
        }
        _ => format!(
            "{stem}.conflict-{timestamp}-{}-{hash_prefix}",
            std::process::id()
        ),
    };
    destination.with_file_name(filename)
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}
