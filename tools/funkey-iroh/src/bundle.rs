use std::{
    collections::HashSet,
    fs as stdfs,
    path::{Component, Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use iroh::{EndpointId, endpoint::Connection};
use tokio::{
    fs::{self, File, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
    time::timeout,
};

use crate::{
    config::{Paths, max_bundle_bytes, max_bundle_files},
    endpoint,
    identity,
    peers::PeerBook,
    save::{hash_file, sanitize_component},
    wire::{
        BUNDLE_ALPN, BundleEntryHeader, BundleHeader, read_bundle_entry,
        read_bundle_header, read_status, write_bundle_entry, write_bundle_header,
        write_status,
    },
};

const COPY_BUFFER_BYTES: usize = 64 * 1024;
const CLOSE_GRACE: Duration = Duration::from_secs(5);
const BUNDLE_ID_FILE: &str = ".funkey-bundle-id";

#[derive(Clone, Debug)]
struct BundleFile {
    source: PathBuf,
    relative: String,
    size: u64,
    hash: [u8; 32],
}

pub async fn inspect(directory: &Path) -> Result<()> {
    let (header, files) = collect_bundle(directory).await?;
    println!("name\t{}", header.name);
    println!("files\t{}", header.files);
    println!("bytes\t{}", header.total_size);
    println!("manifest_blake3\t{}", hex(&header.manifest_hash));
    for file in files {
        println!(
            "file\t{}\t{}\t{}",
            file.relative,
            file.size,
            hex(&file.hash)
        );
    }
    Ok(())
}

pub async fn send(
    paths: Paths,
    peer_name_or_ticket: &str,
    directory: &Path,
) -> Result<()> {
    paths.ensure()?;
    let (header, files) = collect_bundle(directory).await?;

    let peer_book = PeerBook::new(paths.peers.clone());
    let ticket = peer_book.resolve(peer_name_or_ticket)?;
    let remote_id = ticket.endpoint_addr().id.clone();

    let secret = identity::load_or_create(&paths.identity)?;
    let endpoint = endpoint::bind(secret, Vec::new()).await?;
    let connection = endpoint
        .connect(ticket.endpoint_addr().clone(), BUNDLE_ALPN)
        .await
        .with_context(|| format!("connect to bundle peer {remote_id}"))?;
    let (mut send, mut recv) = connection
        .open_bi()
        .await
        .context("open bundle transfer stream")?;

    write_bundle_header(&mut send, &header)
        .await
        .context("send bundle metadata")?;
    let (code, message) = read_status(&mut recv)
        .await
        .context("read bundle receiver preflight response")?;

    match code.as_str() {
        "READY" => {}
        "SKIP" => {
            send.finish().context("finish skipped bundle stream")?;
            println!("already present on {remote_id}: {message}");
            connection.close(0u32.into(), b"already present");
            endpoint.close().await;
            return Ok(());
        }
        "ERROR" => bail!("receiver rejected bundle: {message}"),
        other => bail!("unexpected bundle preflight response {other:?}: {message}"),
    }

    let mut buffer = vec![0u8; COPY_BUFFER_BYTES];
    for file in &files {
        write_bundle_entry(
            &mut send,
            &BundleEntryHeader {
                path: file.relative.clone(),
                size: file.size,
                hash: file.hash,
            },
        )
        .await
        .with_context(|| format!("send metadata for {}", file.relative))?;

        let mut source = File::open(&file.source)
            .await
            .with_context(|| format!("open bundle file {}", file.source.display()))?;
        let mut remaining = file.size;
        while remaining > 0 {
            let wanted = usize::try_from(remaining.min(buffer.len() as u64))
                .expect("bounded by the fixed buffer length");
            let read = source
                .read(&mut buffer[..wanted])
                .await
                .with_context(|| format!("read bundle file {}", file.source.display()))?;
            if read == 0 {
                bail!(
                    "bundle file {} ended with {remaining} bytes missing",
                    file.source.display()
                );
            }
            send.write_all(&buffer[..read])
                .await
                .with_context(|| format!("send bundle file {}", file.relative))?;
            remaining -= read as u64;
        }
    }
    send.finish().context("finish bundle contents")?;

    let (code, message) = read_status(&mut recv)
        .await
        .context("read final bundle receiver response")?;
    match code.as_str() {
        "STORED" => println!("stored bundle on {remote_id}: {message}"),
        "CONFLICT" => println!("stored bundle conflict on {remote_id}: {message}"),
        "ERROR" => bail!("receiver failed to store bundle: {message}"),
        other => bail!("unexpected final bundle response {other:?}: {message}"),
    }

    connection.close(0u32.into(), b"bundle complete");
    endpoint.close().await;
    Ok(())
}

pub(crate) async fn handle_bundle_connection(
    connection: Connection,
    paths: Paths,
    allow_unpaired: bool,
) -> Result<()> {
    let remote_id = connection.remote_id();
    let peer_book = PeerBook::new(paths.peers.clone());
    let paired_name = peer_book.name_for(&remote_id)?;

    if paired_name.is_none() && !allow_unpaired {
        eprintln!("funkey-iroh: refusing unpaired bundle sender {remote_id}");
        connection.close(1u32.into(), b"unpaired endpoint");
        return Ok(());
    }

    let peer_directory = paired_name
        .as_deref()
        .map(sanitize_component)
        .unwrap_or_else(|| short_endpoint_id(&remote_id));

    receive_bundle(connection, &paths, &peer_directory).await
}

async fn receive_bundle(
    connection: Connection,
    paths: &Paths,
    peer_directory: &str,
) -> Result<()> {
    let remote_id = connection.remote_id();
    let (mut send, mut recv) = connection
        .accept_bi()
        .await
        .context("accept bundle transfer stream")?;

    let header = match read_bundle_header(&mut recv).await {
        Ok(header) => header,
        Err(error) => {
            let _ = write_status(
                &mut send,
                "ERROR",
                &format!("invalid bundle header: {error:#}"),
            )
            .await;
            let _ = send.finish();
            return Err(error).context("read bundle transfer header");
        }
    };

    let maximum_bytes = max_bundle_bytes()?;
    let maximum_files = max_bundle_files()?;
    if header.files == 0 {
        write_status(&mut send, "ERROR", "bundle contains no files").await?;
        send.finish().context("finish empty-bundle response")?;
        return Ok(());
    }
    if u64::from(header.files) > maximum_files {
        write_status(
            &mut send,
            "ERROR",
            &format!(
                "bundle contains {} files; receiver limit is {maximum_files}",
                header.files
            ),
        )
        .await?;
        send.finish().context("finish file-count response")?;
        return Ok(());
    }
    if header.total_size > maximum_bytes {
        write_status(
            &mut send,
            "ERROR",
            &format!(
                "bundle is {} bytes; receiver limit is {maximum_bytes}",
                header.total_size
            ),
        )
        .await?;
        send.finish().context("finish oversized-bundle response")?;
        return Ok(());
    }

    let bundle_name = normalize_bundle_name(&header.name);
    let peer_root = paths.bundle_inbox.join(peer_directory);
    fs::create_dir_all(&peer_root)
        .await
        .with_context(|| format!("create bundle inbox {}", peer_root.display()))?;

    let requested_destination = peer_root.join(&bundle_name);
    let requested_id = requested_destination.join(BUNDLE_ID_FILE);
    if let Ok(existing) = fs::read_to_string(&requested_id).await {
        if existing.trim() == hex(&header.manifest_hash) {
            write_status(
                &mut send,
                "SKIP",
                &requested_destination.display().to_string(),
            )
            .await?;
            send.finish().context("finish duplicate-bundle response")?;
            let _ = timeout(CLOSE_GRACE, connection.closed()).await;
            return Ok(());
        }
    }

    let conflict = fs::try_exists(&requested_destination)
        .await
        .with_context(|| format!("check bundle destination {}", requested_destination.display()))?;
    let destination = if conflict {
        conflict_path(&requested_destination, &header.manifest_hash)
    } else {
        requested_destination
    };
    let temporary = temporary_directory(&destination);
    let _ = fs::remove_dir_all(&temporary).await;
    fs::create_dir(&temporary)
        .await
        .with_context(|| format!("create bundle staging directory {}", temporary.display()))?;

    write_status(&mut send, "READY", "").await?;

    let receive_result = receive_entries(&mut recv, &temporary, &header).await;
    if let Err(error) = receive_result {
        let _ = fs::remove_dir_all(&temporary).await;
        let _ = write_status(&mut send, "ERROR", &format!("{error:#}")).await;
        let _ = send.finish();
        return Err(error);
    }

    let bundle_id = format!("{}\n", hex(&header.manifest_hash));
    let id_path = temporary.join(BUNDLE_ID_FILE);
    fs::write(&id_path, bundle_id.as_bytes())
        .await
        .with_context(|| format!("write bundle identifier {}", id_path.display()))?;
    let id_file = OpenOptions::new()
        .read(true)
        .open(&id_path)
        .await
        .with_context(|| format!("open bundle identifier {}", id_path.display()))?;
    id_file
        .sync_all()
        .await
        .with_context(|| format!("sync bundle identifier {}", id_path.display()))?;
    sync_directory(&temporary)?;

    fs::rename(&temporary, &destination).await.with_context(|| {
        format!(
            "atomically install received bundle {} as {}",
            temporary.display(),
            destination.display()
        )
    })?;
    sync_directory(&peer_root)?;

    let code = if conflict { "CONFLICT" } else { "STORED" };
    write_status(&mut send, code, &destination.display().to_string()).await?;
    send.finish().context("finish final bundle response")?;

    eprintln!(
        "funkey-iroh: received {} files / {} bytes from {remote_id} into {}",
        header.files,
        header.total_size,
        destination.display()
    );
    let _ = timeout(CLOSE_GRACE, connection.closed()).await;
    Ok(())
}

async fn receive_entries<R>(
    reader: &mut R,
    temporary: &Path,
    header: &BundleHeader,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut seen = HashSet::new();
    let mut total = 0u64;
    let mut manifest_hasher = blake3::Hasher::new();

    for _ in 0..header.files {
        let entry = read_bundle_entry(reader)
            .await
            .context("read bundle entry metadata")?;
        validate_relative_path(&entry.path)?;
        if entry.path == BUNDLE_ID_FILE || !seen.insert(entry.path.clone()) {
            bail!("duplicate or reserved bundle path {:?}", entry.path);
        }

        total = total
            .checked_add(entry.size)
            .ok_or_else(|| anyhow::anyhow!("bundle byte count overflow"))?;
        if total > header.total_size || total > max_bundle_bytes()? {
            bail!("bundle entries exceed declared or configured byte limit");
        }

        update_manifest_hash(
            &mut manifest_hasher,
            &entry.path,
            entry.size,
            &entry.hash,
        );

        let destination = temporary.join(&entry.path);
        let parent = destination
            .parent()
            .ok_or_else(|| anyhow::anyhow!("bundle path has no parent"))?;
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("create bundle directory {}", parent.display()))?;
        receive_file(reader, &destination, entry.size, &entry.hash).await?;
    }

    if total != header.total_size {
        bail!(
            "bundle declared {} bytes but entries total {total}",
            header.total_size
        );
    }
    let actual_manifest = *manifest_hasher.finalize().as_bytes();
    if actual_manifest != header.manifest_hash {
        bail!(
            "bundle manifest BLAKE3 mismatch: expected {}, received {}",
            hex(&header.manifest_hash),
            hex(&actual_manifest)
        );
    }
    Ok(())
}

async fn receive_file<R>(
    reader: &mut R,
    destination: &Path,
    size: u64,
    expected_hash: &[u8; 32],
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .await
        .with_context(|| format!("create incoming bundle file {}", destination.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut remaining = size;
    let mut buffer = vec![0u8; COPY_BUFFER_BYTES];

    while remaining > 0 {
        let wanted = usize::try_from(remaining.min(buffer.len() as u64))
            .expect("bounded by the fixed buffer length");
        let read = reader
            .read(&mut buffer[..wanted])
            .await
            .with_context(|| format!("read incoming bundle file {}", destination.display()))?;
        if read == 0 {
            bail!(
                "incoming bundle file {} ended with {remaining} bytes missing",
                destination.display()
            );
        }
        file.write_all(&buffer[..read])
            .await
            .with_context(|| format!("write incoming bundle file {}", destination.display()))?;
        hasher.update(&buffer[..read]);
        remaining -= read as u64;
    }
    file.sync_all()
        .await
        .with_context(|| format!("sync incoming bundle file {}", destination.display()))?;
    drop(file);

    let actual_hash = *hasher.finalize().as_bytes();
    if &actual_hash != expected_hash {
        bail!(
            "BLAKE3 mismatch for {}: expected {}, received {}",
            destination.display(),
            hex(expected_hash),
            hex(&actual_hash)
        );
    }
    Ok(())
}

async fn collect_bundle(directory: &Path) -> Result<(BundleHeader, Vec<BundleFile>)> {
    let metadata = stdfs::symlink_metadata(directory)
        .with_context(|| format!("stat bundle directory {}", directory.display()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        bail!(
            "bundle source must be a real directory, not a symlink: {}",
            directory.display()
        );
    }

    let name = directory
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("bundle directory name is not valid UTF-8"))?
        .to_owned();
    if !name.ends_with(".funkey") {
        bail!("portable bundle directory must end in .funkey: {name:?}");
    }

    let mut paths = Vec::new();
    collect_paths(directory, directory, &mut paths)?;
    paths.sort_by(|left, right| left.0.cmp(&right.0));

    let maximum_files = max_bundle_files()?;
    if paths.is_empty() {
        bail!("bundle contains no regular files: {}", directory.display());
    }
    if paths.len() as u64 > maximum_files {
        bail!(
            "bundle contains {} files, over FUNKEY_IROH_MAX_BUNDLE_FILES={maximum_files}",
            paths.len()
        );
    }

    let maximum_bytes = max_bundle_bytes()?;
    let mut files = Vec::with_capacity(paths.len());
    let mut total_size = 0u64;
    let mut manifest_hasher = blake3::Hasher::new();

    for (relative, source) in paths {
        let (hash, size) = hash_file(&source).await?;
        total_size = total_size
            .checked_add(size)
            .ok_or_else(|| anyhow::anyhow!("bundle byte count overflow"))?;
        if total_size > maximum_bytes {
            bail!(
                "bundle is larger than FUNKEY_IROH_MAX_BUNDLE_BYTES={maximum_bytes}"
            );
        }
        update_manifest_hash(&mut manifest_hasher, &relative, size, &hash);
        files.push(BundleFile {
            source,
            relative,
            size,
            hash,
        });
    }

    let files_count = u32::try_from(files.len()).context("bundle file count does not fit u32")?;
    Ok((
        BundleHeader {
            name,
            files: files_count,
            total_size,
            manifest_hash: *manifest_hasher.finalize().as_bytes(),
        },
        files,
    ))
}

fn collect_paths(
    root: &Path,
    directory: &Path,
    output: &mut Vec<(String, PathBuf)>,
) -> Result<()> {
    let mut entries = stdfs::read_dir(directory)
        .with_context(|| format!("read bundle directory {}", directory.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("enumerate bundle directory {}", directory.display()))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let metadata = stdfs::symlink_metadata(&path)
            .with_context(|| format!("stat bundle entry {}", path.display()))?;
        if metadata.file_type().is_symlink() {
            bail!("bundle may not contain symlinks: {}", path.display());
        }
        if metadata.is_dir() {
            collect_paths(root, &path, output)?;
            continue;
        }
        if !metadata.is_file() {
            bail!("bundle contains a non-regular entry: {}", path.display());
        }

        let relative_path = path
            .strip_prefix(root)
            .with_context(|| format!("make {} relative to {}", path.display(), root.display()))?;
        let relative = relative_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("bundle path is not valid UTF-8: {}", path.display()))?
            .replace(std::path::MAIN_SEPARATOR, "/");
        validate_relative_path(&relative)?;
        if relative == BUNDLE_ID_FILE
            || relative
                .split('/')
                .any(|component| component.starts_with(".part-"))
        {
            continue;
        }
        output.push((relative, path));
    }
    Ok(())
}

pub fn validate_relative_path(value: &str) -> Result<()> {
    if value.is_empty()
        || value.starts_with('/')
        || value.ends_with('/')
        || value.contains('\\')
        || value.as_bytes().contains(&0)
    {
        bail!("unsafe portable bundle path {value:?}");
    }

    let path = Path::new(value);
    let mut components = 0usize;
    for component in path.components() {
        match component {
            Component::Normal(name) if !name.is_empty() => components += 1,
            _ => bail!("unsafe portable bundle path {value:?}"),
        }
    }
    if components == 0 || components > 32 {
        bail!("portable bundle path has an invalid component count: {value:?}");
    }
    Ok(())
}

fn update_manifest_hash(
    hasher: &mut blake3::Hasher,
    path: &str,
    size: u64,
    hash: &[u8; 32],
) {
    hasher.update(&(path.len() as u32).to_be_bytes());
    hasher.update(path.as_bytes());
    hasher.update(&size.to_be_bytes());
    hasher.update(hash);
}

fn normalize_bundle_name(input: &str) -> String {
    let mut name = sanitize_component(input);
    if !name.ends_with(".funkey") {
        name.push_str(".funkey");
    }
    name
}

fn conflict_path(destination: &Path, hash: &[u8; 32]) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let hash_prefix = &hex(hash)[..12];
    let name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("bundle.funkey");
    let stem = name.strip_suffix(".funkey").unwrap_or(name);
    destination.with_file_name(format!("{stem}.conflict-{timestamp}-{hash_prefix}.funkey"))
}

fn short_endpoint_id(endpoint_id: &EndpointId) -> String {
    endpoint_id.to_string().chars().take(16).collect()
}

fn temporary_directory(destination: &Path) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("bundle.funkey");
    destination.with_file_name(format!(".part-{name}-{}-{timestamp}", std::process::id()))
}

fn sync_directory(path: &Path) -> Result<()> {
    let directory = stdfs::File::open(path)
        .with_context(|| format!("open directory for sync {}", path.display()))?;
    match directory.sync_all() {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::InvalidInput
                    | std::io::ErrorKind::Unsupported
            ) =>
        {
            // Some FAT/filesystem combinations do not implement directory
            // fsync. Every received regular file and the bundle id are still
            // fsynced before the atomic rename.
            Ok(())
        }
        Err(error) => Err(error)
            .with_context(|| format!("sync directory {}", path.display())),
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_paths_are_strictly_relative() {
        for good in [
            "manifest.json",
            "content/Pokemon Crystal.gbc",
            "retroarch/states/Pokemon Crystal.state0",
        ] {
            validate_relative_path(good).unwrap();
        }

        for bad in [
            "",
            "/etc/passwd",
            "../escape",
            "content/../../escape",
            "content\\escape",
            "content/",
            "./content",
        ] {
            assert!(validate_relative_path(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn normalized_names_keep_bundle_suffix() {
        assert_eq!(
            normalize_bundle_name("Pokemon Crystal.funkey"),
            "Pokemon_Crystal.funkey"
        );
        assert_eq!(normalize_bundle_name("Metroid"), "Metroid.funkey");
    }

    #[test]
    fn endpoint_fallback_is_short_and_stable() {
        let key = iroh::SecretKey::generate();
        let endpoint_id: EndpointId = key.public();
        assert_eq!(short_endpoint_id(&endpoint_id).len(), 16);
    }
}
