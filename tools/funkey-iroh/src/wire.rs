use anyhow::{Context, Result, bail};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const SAVE_ALPN: &[u8] = b"funkey/saves/1";
pub const BUNDLE_ALPN: &[u8] = b"funkey/bundles/1";
pub const NETPLAY_ALPN: &[u8] = b"funkey/netplay/1";

const SAVE_MAGIC: &[u8; 8] = b"FKSAVE01";
const BUNDLE_MAGIC: &[u8; 8] = b"FKBNDL01";
const MAX_FIELD_BYTES: usize = 255;
const MAX_BUNDLE_PATH_BYTES: usize = 1024;
const MAX_STATUS_BYTES: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveHeader {
    pub system: String,
    pub game: String,
    pub filename: String,
    pub size: u64,
    pub hash: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleHeader {
    pub name: String,
    pub files: u32,
    pub total_size: u64,
    pub manifest_hash: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleEntryHeader {
    pub path: String,
    pub size: u64,
    pub hash: [u8; 32],
}

pub async fn write_save_header<W>(writer: &mut W, header: &SaveHeader) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    let fields = [
        ("system", header.system.as_bytes()),
        ("game", header.game.as_bytes()),
        ("filename", header.filename.as_bytes()),
    ];
    for (name, bytes) in fields {
        validate_field(name, bytes, MAX_FIELD_BYTES)?;
    }

    writer.write_all(SAVE_MAGIC).await?;
    write_u16(writer, header.system.len()).await?;
    write_u16(writer, header.game.len()).await?;
    write_u16(writer, header.filename.len()).await?;
    writer.write_all(&header.size.to_be_bytes()).await?;
    writer.write_all(&header.hash).await?;
    writer.write_all(header.system.as_bytes()).await?;
    writer.write_all(header.game.as_bytes()).await?;
    writer.write_all(header.filename.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

pub async fn read_save_header<R>(reader: &mut R) -> Result<SaveHeader>
where
    R: AsyncRead + Unpin,
{
    read_magic(reader, SAVE_MAGIC, "save").await?;
    let system_len = read_u16(reader).await? as usize;
    let game_len = read_u16(reader).await? as usize;
    let filename_len = read_u16(reader).await? as usize;

    for (name, len) in [
        ("system", system_len),
        ("game", game_len),
        ("filename", filename_len),
    ] {
        validate_length(name, len, MAX_FIELD_BYTES)?;
    }

    let size = read_u64(reader).await.context("read save size")?;
    let hash = read_hash(reader, "save BLAKE3 hash").await?;
    let system = read_utf8(reader, system_len, "system").await?;
    let game = read_utf8(reader, game_len, "game").await?;
    let filename = read_utf8(reader, filename_len, "filename").await?;

    Ok(SaveHeader {
        system,
        game,
        filename,
        size,
        hash,
    })
}

pub async fn write_header<W>(writer: &mut W, header: &SaveHeader) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    write_save_header(writer, header).await
}

pub async fn read_header<R>(reader: &mut R) -> Result<SaveHeader>
where
    R: AsyncRead + Unpin,
{
    read_save_header(reader).await
}

pub async fn write_bundle_header<W>(writer: &mut W, header: &BundleHeader) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    validate_field("bundle name", header.name.as_bytes(), MAX_FIELD_BYTES)?;
    writer.write_all(BUNDLE_MAGIC).await?;
    write_u16(writer, header.name.len()).await?;
    writer.write_all(&header.files.to_be_bytes()).await?;
    writer.write_all(&header.total_size.to_be_bytes()).await?;
    writer.write_all(&header.manifest_hash).await?;
    writer.write_all(header.name.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

pub async fn read_bundle_header<R>(reader: &mut R) -> Result<BundleHeader>
where
    R: AsyncRead + Unpin,
{
    read_magic(reader, BUNDLE_MAGIC, "bundle").await?;
    let name_len = read_u16(reader).await? as usize;
    validate_length("bundle name", name_len, MAX_FIELD_BYTES)?;
    let files = read_u32(reader).await.context("read bundle file count")?;
    let total_size = read_u64(reader).await.context("read bundle total size")?;
    let manifest_hash = read_hash(reader, "bundle manifest hash").await?;
    let name = read_utf8(reader, name_len, "bundle name").await?;

    Ok(BundleHeader {
        name,
        files,
        total_size,
        manifest_hash,
    })
}

pub async fn write_bundle_entry<W>(
    writer: &mut W,
    header: &BundleEntryHeader,
) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    validate_field(
        "bundle relative path",
        header.path.as_bytes(),
        MAX_BUNDLE_PATH_BYTES,
    )?;
    write_u16(writer, header.path.len()).await?;
    writer.write_all(&header.size.to_be_bytes()).await?;
    writer.write_all(&header.hash).await?;
    writer.write_all(header.path.as_bytes()).await?;
    Ok(())
}

pub async fn read_bundle_entry<R>(reader: &mut R) -> Result<BundleEntryHeader>
where
    R: AsyncRead + Unpin,
{
    let path_len = read_u16(reader).await? as usize;
    validate_length("bundle relative path", path_len, MAX_BUNDLE_PATH_BYTES)?;
    let size = read_u64(reader).await.context("read bundle entry size")?;
    let hash = read_hash(reader, "bundle entry BLAKE3 hash").await?;
    let path = read_utf8(reader, path_len, "bundle relative path").await?;
    Ok(BundleEntryHeader { path, size, hash })
}

pub async fn write_status<W>(writer: &mut W, code: &str, message: &str) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    validate_status_part(code, "status code")?;
    let clean_message = message
        .replace('\r', " ")
        .replace('\n', " ")
        .replace('\t', " ");
    if clean_message.len() + code.len() + 2 > MAX_STATUS_BYTES {
        bail!("status response is too long");
    }
    writer.write_all(code.as_bytes()).await?;
    if clean_message.is_empty() {
        writer.write_all(b"\n").await?;
    } else {
        writer.write_all(b"\t").await?;
        writer.write_all(clean_message.as_bytes()).await?;
        writer.write_all(b"\n").await?;
    }
    writer.flush().await?;
    Ok(())
}

pub async fn read_status<R>(reader: &mut R) -> Result<(String, String)>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = Vec::with_capacity(64);
    loop {
        if bytes.len() >= MAX_STATUS_BYTES {
            bail!("status response exceeded {MAX_STATUS_BYTES} bytes");
        }
        let mut byte = [0u8; 1];
        let read = reader.read(&mut byte).await?;
        if read == 0 {
            bail!("peer closed the stream before sending a status response");
        }
        if byte[0] == b'\n' {
            break;
        }
        if byte[0] != b'\r' {
            bytes.push(byte[0]);
        }
    }

    let line = String::from_utf8(bytes).context("status response was not UTF-8")?;
    let (code, message) = line
        .split_once('\t')
        .map_or((line.as_str(), ""), |(code, message)| (code, message));
    validate_status_part(code, "status code")?;
    Ok((code.to_owned(), message.to_owned()))
}

async fn read_magic<R>(reader: &mut R, expected: &[u8; 8], label: &str) -> Result<()>
where
    R: AsyncRead + Unpin,
{
    let mut magic = [0u8; 8];
    reader
        .read_exact(&mut magic)
        .await
        .with_context(|| format!("read {label} protocol magic"))?;
    if &magic != expected {
        bail!("invalid {label} protocol magic");
    }
    Ok(())
}

async fn write_u16<W>(writer: &mut W, value: usize) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    let value = u16::try_from(value).context("field length does not fit u16")?;
    writer.write_all(&value.to_be_bytes()).await?;
    Ok(())
}

async fn read_u16<R>(reader: &mut R) -> Result<u16>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = [0u8; 2];
    reader.read_exact(&mut bytes).await?;
    Ok(u16::from_be_bytes(bytes))
}

async fn read_u32<R>(reader: &mut R) -> Result<u32>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = [0u8; 4];
    reader.read_exact(&mut bytes).await?;
    Ok(u32::from_be_bytes(bytes))
}

async fn read_u64<R>(reader: &mut R) -> Result<u64>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = [0u8; 8];
    reader.read_exact(&mut bytes).await?;
    Ok(u64::from_be_bytes(bytes))
}

async fn read_hash<R>(reader: &mut R, label: &str) -> Result<[u8; 32]>
where
    R: AsyncRead + Unpin,
{
    let mut hash = [0u8; 32];
    reader
        .read_exact(&mut hash)
        .await
        .with_context(|| format!("read {label}"))?;
    Ok(hash)
}

async fn read_utf8<R>(reader: &mut R, len: usize, name: &str) -> Result<String>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = vec![0u8; len];
    reader
        .read_exact(&mut bytes)
        .await
        .with_context(|| format!("read {name} field"))?;
    String::from_utf8(bytes).with_context(|| format!("{name} field was not UTF-8"))
}

fn validate_field(name: &str, bytes: &[u8], maximum: usize) -> Result<()> {
    validate_length(name, bytes.len(), maximum)
}

fn validate_length(name: &str, len: usize, maximum: usize) -> Result<()> {
    if len == 0 || len > maximum {
        bail!("{name} must contain between 1 and {maximum} UTF-8 bytes");
    }
    Ok(())
}

fn validate_status_part(value: &str, name: &str) -> Result<()> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte == b'_')
    {
        bail!("{name} must contain only ASCII uppercase letters and underscores");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn save_header_round_trips() {
        let expected = SaveHeader {
            system: "gbc".to_owned(),
            game: "Pokemon Crystal".to_owned(),
            filename: "Pokemon Crystal.sav".to_owned(),
            size: 32_768,
            hash: [0x5a; 32],
        };

        let (mut left, mut right) = tokio::io::duplex(2048);
        let to_write = expected.clone();
        let writer = tokio::spawn(async move {
            write_save_header(&mut left, &to_write).await.unwrap();
        });
        let actual = read_save_header(&mut right).await.unwrap();
        writer.await.unwrap();
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn bundle_headers_round_trip() {
        let bundle = BundleHeader {
            name: "Pokemon_Crystal.funkey".to_owned(),
            files: 4,
            total_size: 1_234_567,
            manifest_hash: [0x11; 32],
        };
        let entry = BundleEntryHeader {
            path: "retroarch/states/Pokemon Crystal.state0".to_owned(),
            size: 98_765,
            hash: [0x22; 32],
        };

        let (mut left, mut right) = tokio::io::duplex(4096);
        let bundle_to_write = bundle.clone();
        let entry_to_write = entry.clone();
        let writer = tokio::spawn(async move {
            write_bundle_header(&mut left, &bundle_to_write)
                .await
                .unwrap();
            write_bundle_entry(&mut left, &entry_to_write)
                .await
                .unwrap();
        });
        assert_eq!(read_bundle_header(&mut right).await.unwrap(), bundle);
        assert_eq!(read_bundle_entry(&mut right).await.unwrap(), entry);
        writer.await.unwrap();
    }

    #[tokio::test]
    async fn status_round_trips_and_scrubs_controls() {
        let (mut left, mut right) = tokio::io::duplex(256);
        let writer = tokio::spawn(async move {
            write_status(&mut left, "STORED", "a\tb\nc")
                .await
                .unwrap();
        });
        let actual = read_status(&mut right).await.unwrap();
        writer.await.unwrap();
        assert_eq!(actual, ("STORED".to_owned(), "a b c".to_owned()));
    }
}
