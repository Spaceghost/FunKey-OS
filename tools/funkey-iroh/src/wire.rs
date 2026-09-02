use anyhow::{Context, Result, bail};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const SAVE_ALPN: &[u8] = b"funkey/saves/1";
pub const NETPLAY_ALPN: &[u8] = b"funkey/netplay/1";
const SAVE_MAGIC: &[u8; 8] = b"FKSAVE01";
const MAX_FIELD_BYTES: usize = 255;
const MAX_STATUS_BYTES: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveHeader {
    pub system: String,
    pub game: String,
    pub filename: String,
    pub size: u64,
    pub hash: [u8; 32],
}

pub async fn write_header<W>(writer: &mut W, header: &SaveHeader) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    let fields = [
        ("system", header.system.as_bytes()),
        ("game", header.game.as_bytes()),
        ("filename", header.filename.as_bytes()),
    ];
    for (name, bytes) in fields {
        if bytes.is_empty() || bytes.len() > MAX_FIELD_BYTES {
            bail!(
                "{name} must contain between 1 and {MAX_FIELD_BYTES} UTF-8 bytes"
            );
        }
    }

    writer.write_all(SAVE_MAGIC).await?;
    writer
        .write_all(&(header.system.len() as u16).to_be_bytes())
        .await?;
    writer
        .write_all(&(header.game.len() as u16).to_be_bytes())
        .await?;
    writer
        .write_all(&(header.filename.len() as u16).to_be_bytes())
        .await?;
    writer.write_all(&header.size.to_be_bytes()).await?;
    writer.write_all(&header.hash).await?;
    writer.write_all(header.system.as_bytes()).await?;
    writer.write_all(header.game.as_bytes()).await?;
    writer.write_all(header.filename.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

pub async fn read_header<R>(reader: &mut R) -> Result<SaveHeader>
where
    R: AsyncRead + Unpin,
{
    let mut magic = [0u8; SAVE_MAGIC.len()];
    reader
        .read_exact(&mut magic)
        .await
        .context("read save protocol magic")?;
    if &magic != SAVE_MAGIC {
        bail!("invalid save protocol magic");
    }

    let system_len = read_u16(reader).await? as usize;
    let game_len = read_u16(reader).await? as usize;
    let filename_len = read_u16(reader).await? as usize;

    for (name, len) in [
        ("system", system_len),
        ("game", game_len),
        ("filename", filename_len),
    ] {
        if len == 0 || len > MAX_FIELD_BYTES {
            bail!(
                "invalid {name} length {len}; expected between 1 and {MAX_FIELD_BYTES}"
            );
        }
    }

    let mut size = [0u8; 8];
    reader.read_exact(&mut size).await.context("read save size")?;

    let mut hash = [0u8; 32];
    reader
        .read_exact(&mut hash)
        .await
        .context("read save BLAKE3 hash")?;

    let system = read_utf8(reader, system_len, "system").await?;
    let game = read_utf8(reader, game_len, "game").await?;
    let filename = read_utf8(reader, filename_len, "filename").await?;

    Ok(SaveHeader {
        system,
        game,
        filename,
        size: u64::from_be_bytes(size),
        hash,
    })
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
    if clean_message.is_empty() {
        writer.write_all(code.as_bytes()).await?;
        writer.write_all(b"\n").await?;
    } else {
        writer.write_all(code.as_bytes()).await?;
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

async fn read_u16<R>(reader: &mut R) -> Result<u16>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = [0u8; 2];
    reader.read_exact(&mut bytes).await?;
    Ok(u16::from_be_bytes(bytes))
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
            write_header(&mut left, &to_write).await.unwrap();
        });
        let actual = read_header(&mut right).await.unwrap();
        writer.await.unwrap();
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn status_round_trips_and_scrubs_controls() {
        let (mut left, mut right) = tokio::io::duplex(256);
        let writer = tokio::spawn(async move {
            write_status(&mut left, "STORED", "a\tb\nc").await.unwrap();
        });
        let actual = read_status(&mut right).await.unwrap();
        writer.await.unwrap();
        assert_eq!(actual, ("STORED".to_owned(), "a b c".to_owned()));
    }
}
