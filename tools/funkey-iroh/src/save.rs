use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use iroh::{EndpointId, endpoint::Connection};
use tokio::{
    fs::{self, File, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
    signal,
    time::timeout,
};

use crate::{
    config::{Paths, max_save_bytes, online_timeout},
    endpoint,
    identity,
    peers::PeerBook,
    wire::{SAVE_ALPN, SaveHeader, read_header, read_status, write_header, write_status},
};

const COPY_BUFFER_BYTES: usize = 64 * 1024;
const CLOSE_GRACE: Duration = Duration::from_secs(5);

pub async fn serve(paths: Paths, allow_unpaired: bool) -> Result<()> {
    paths.ensure()?;
    let secret = identity::load_or_create(&paths.identity)?;
    let endpoint = endpoint::bind(secret, vec![SAVE_ALPN.to_vec()]).await?;
    let ticket = endpoint::ticket(&endpoint, &paths.current_ticket, online_timeout()?).await?;

    println!("{ticket}");
    eprintln!(
        "funkey-iroh: save receiver online as {} (paired_only={})",
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
                    if let Err(error) =
                        handle_save_connection(connection, connection_paths, allow_unpaired).await
                    {
                        eprintln!("funkey-iroh: save receive failed: {error:#}");
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

pub async fn send(
    paths: Paths,
    peer_name_or_ticket: &str,
    system: &str,
    game: &str,
    source: &Path,
) -> Result<()> {
    paths.ensure()?;
    let metadata = fs::metadata(source)
        .await
        .with_context(|| format!("stat save {}", source.display()))?;
    if !metadata.is_file() {
        bail!("save source is not a regular file: {}", source.display());
    }

    let maximum = max_save_bytes()?;
    if metadata.len() > maximum {
        bail!(
            "save is {} bytes, larger than FUNKEY_IROH_MAX_SAVE_BYTES={maximum}",
            metadata.len()
        );
    }

    let filename = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("save filename is not valid UTF-8: {}", source.display()))?
        .to_owned();

    let (hash, size) = hash_file(source).await?;
    if size != metadata.len() {
        bail!(
            "save changed while hashing: metadata said {} bytes, read {size}",
            metadata.len()
        );
    }

    let header = SaveHeader {
        system: system.to_owned(),
        game: game.to_owned(),
        filename,
        size,
        hash,
    };

    let peer_book = PeerBook::new(paths.peers.clone());
    let ticket = peer_book.resolve(peer_name_or_ticket)?;
    let remote_id = ticket.endpoint_addr().id.clone();

    let secret = identity::load_or_create(&paths.identity)?;
    let endpoint = endpoint::bind(secret, Vec::new()).await?;
    let connection = endpoint
        .connect(ticket.endpoint_addr().clone(), SAVE_ALPN)
        .await
        .with_context(|| format!("connect to save peer {remote_id}"))?;
    let (mut send, mut recv) = connection
        .open_bi()
        .await
        .context("open save transfer stream")?;

    write_header(&mut send, &header)
        .await
        .context("send save metadata")?;

    let (code, message) = read_status(&mut recv)
        .await
        .context("read save receiver preflight response")?;

    match code.as_str() {
        "READY" => {}
        "SKIP" => {
            send.finish().context("finish skipped save stream")?;
            println!(
                "already present on {remote_id}: {}",
                if message.is_empty() { source.display().to_string() } else { message }
            );
            connection.close(0u32.into(), b"already present");
            endpoint.close().await;
            return Ok(());
        }
        "ERROR" => {
            bail!("receiver rejected save: {message}");
        }
        other => bail!("unexpected save preflight response {other:?}: {message}"),
    }

    let mut file = File::open(source)
        .await
        .with_context(|| format!("open save for transfer {}", source.display()))?;
    let mut remaining = size;
    let mut buffer = vec![0u8; COPY_BUFFER_BYTES];

    while remaining > 0 {
        let wanted = usize::try_from(remaining.min(buffer.len() as u64))
            .expect("bounded by the fixed buffer length");
        let read = file
            .read(&mut buffer[..wanted])
            .await
            .with_context(|| format!("read save {}", source.display()))?;
        if read == 0 {
            bail!(
                "save ended early while sending; {} bytes remained",
                remaining
            );
        }
        send.write_all(&buffer[..read])
            .await
            .context("send save contents")?;
        remaining -= read as u64;
    }
    send.finish().context("finish save contents")?;

    let (code, message) = read_status(&mut recv)
        .await
        .context("read final save receiver response")?;

    match code.as_str() {
        "STORED" => println!("stored on {remote_id}: {message}"),
        "CONFLICT" => println!("stored conflict copy on {remote_id}: {message}"),
        "ERROR" => bail!("receiver failed to store save: {message}"),
        other => bail!("unexpected final save response {other:?}: {message}"),
    }

    connection.close(0u32.into(), b"save complete");
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

    eprintln!("funkey-iroh: accepting save stream from {remote_id}");
    receive_save(connection, &paths, &peer_directory).await
}

async fn receive_save(connection: Connection, paths: &Paths, peer_directory: &str) -> Result<()> {
    let remote_id = connection.remote_id();
    let (mut send, mut recv) = connection
        .accept_bi()
        .await
        .context("accept save transfer stream")?;

    let header = match read_header(&mut recv).await {
        Ok(header) => header,
        Err(error) => {
            let _ = write_status(&mut send, "ERROR", &format!("invalid header: {error:#}")).await;
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

    let receive_result = receive_file(&mut recv, &temporary, header.size, &header.hash).await;
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

pub async fn hash_file(path: &Path) -> Result<([u8; 32], u64)> {
    let mut file = File::open(path)
        .await
        .with_context(|| format!("open {} for hashing", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut total = 0u64;
    let mut buffer = vec![0u8; COPY_BUFFER_BYTES];

    loop {
        let read = file
            .read(&mut buffer)
            .await
            .with_context(|| format!("hash {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        total += read as u64;
    }

    Ok((*hasher.finalize().as_bytes(), total))
}

pub fn sanitize_component(input: &str) -> String {
    const MAX: usize = 96;
    let mut output = String::with_capacity(input.len().min(MAX));
    let mut previous_separator = false;

    for character in input.chars() {
        if output.len() >= MAX {
            break;
        }
        let mapped = if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
            Some(character)
        } else if character.is_whitespace() {
            Some('_')
        } else {
            Some('-')
        };

        if let Some(character) = mapped {
            let separator = matches!(character, '-' | '_');
            if separator && previous_separator {
                continue;
            }
            output.push(character);
            previous_separator = separator;
        }
    }

    while matches!(output.chars().last(), Some('.' | '-' | '_')) {
        output.pop();
    }
    while output.starts_with('.') {
        output.remove(0);
    }

    if output.is_empty() || output == "." || output == ".." {
        "unnamed".to_owned()
    } else {
        output
    }
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
    destination.with_file_name(format!(".{filename}.part-{}-{sequence}", std::process::id()))
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
    let extension = destination.extension().and_then(|extension| extension.to_str());

    let filename = match extension {
        Some(extension) if !extension.is_empty() => {
            format!("{stem}.conflict-{timestamp}-{}-{hash_prefix}.{extension}", std::process::id())
        }
        _ => format!("{stem}.conflict-{timestamp}-{}-{hash_prefix}", std::process::id()),
    };
    destination.with_file_name(filename)
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_remote_path_components() {
        assert_eq!(sanitize_component("Pokemon Crystal"), "Pokemon_Crystal");
        assert_eq!(sanitize_component("../../etc/passwd"), "etc-passwd");
        assert_eq!(sanitize_component("a///b"), "a-b");
        assert_eq!(sanitize_component("..."), "unnamed");
        assert_eq!(sanitize_component("日本語"), "unnamed");
    }

    #[test]
    fn conflict_name_preserves_extension() {
        let path = conflict_path(Path::new("/tmp/game.sav"), &[0xab; 32]);
        let name = path.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with("game.conflict-"));
        assert!(name.ends_with("-abababababab.sav"));
    }
}
