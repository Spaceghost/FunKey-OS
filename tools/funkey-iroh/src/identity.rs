use std::{
    fs::{self, OpenOptions},
    io::{ErrorKind, Read, Write},
    path::Path,
};

use anyhow::{Context, Result, bail};
use iroh::SecretKey;

pub fn load_or_create(path: &Path) -> Result<SecretKey> {
    match load(path) {
        Ok(key) => return Ok(key),
        Err(error) if error.downcast_ref::<std::io::Error>().is_some_and(|io| io.kind() == ErrorKind::NotFound) => {}
        Err(error) => return Err(error),
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create identity directory {}", parent.display()))?;
    }

    let key = SecretKey::generate();
    let temporary = path.with_extension(format!("tmp.{}", std::process::id()));

    let write_result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .with_context(|| format!("create temporary identity {}", temporary.display()))?;
        file.write_all(&key.to_bytes())
            .with_context(|| format!("write temporary identity {}", temporary.display()))?;
        file.sync_all()
            .with_context(|| format!("sync temporary identity {}", temporary.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // The stock FunKey data partition is FAT, where chmod may be rejected or
            // ignored. Keep atomic identity creation working there, while still applying
            // 0600 when the configured state directory lives on a Unix filesystem.
            if let Err(error) =
                fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))
            {
                eprintln!(
                    "funkey-iroh: could not restrict identity permissions on {}: {error}; physical access to the data partition can expose this key",
                    temporary.display()
                );
            }
        }

        if path.exists() {
            bail!(
                "identity appeared while creating {}; refusing to overwrite it",
                path.display()
            );
        }
        fs::rename(&temporary, path).with_context(|| {
            format!(
                "install generated identity {} as {}",
                temporary.display(),
                path.display()
            )
        })?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result?;

    Ok(key)
}

pub fn load(path: &Path) -> Result<SecretKey> {
    let mut file = fs::File::open(path)
        .with_context(|| format!("open persistent Iroh identity {}", path.display()))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .with_context(|| format!("read persistent Iroh identity {}", path.display()))?;
    if bytes.len() != 32 {
        bail!(
            "persistent Iroh identity {} is {} bytes, expected exactly 32; refusing to regenerate",
            path.display(),
            bytes.len()
        );
    }
    let bytes: [u8; 32] = bytes
        .try_into()
        .expect("length was checked immediately above");
    Ok(SecretKey::from_bytes(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "funkey-iroh-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn identity_is_stable() {
        let directory = unique_path("identity");
        let path = directory.join("key");
        let first = load_or_create(&path).unwrap();
        let second = load_or_create(&path).unwrap();
        assert_eq!(first.public(), second.public());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn corrupt_identity_is_not_replaced() {
        let directory = unique_path("corrupt");
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("key");
        fs::write(&path, b"short").unwrap();
        let error = load_or_create(&path).unwrap_err().to_string();
        assert!(error.contains("expected exactly 32"));
        assert_eq!(fs::read(&path).unwrap(), b"short");
        fs::remove_dir_all(directory).unwrap();
    }
}
