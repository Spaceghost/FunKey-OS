use std::{
    env,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail};

pub const DEFAULT_STATE_DIR: &str = "/mnt/.funkey-iroh";
pub const DEFAULT_LIBRARY_DIR: &str = "/mnt/FunKey/Shared Games";
pub const DEFAULT_MAX_SAVE_BYTES: u64 = 64 * 1024 * 1024;
pub const DEFAULT_MAX_BUNDLE_BYTES: u64 = 16 * 1024 * 1024 * 1024;
pub const DEFAULT_MAX_BUNDLE_FILES: u64 = 4096;
pub const DEFAULT_MAX_INCOMING_CONNECTIONS: u64 = 4;
pub const DEFAULT_HANDSHAKE_TIMEOUT_SECS: u64 = 15;
pub const DEFAULT_ONLINE_TIMEOUT_SECS: u64 = 8;

#[derive(Clone, Debug)]
pub struct Paths {
    pub state_dir: PathBuf,
    pub identity: PathBuf,
    pub peers: PathBuf,
    pub inbox: PathBuf,
    pub bundle_inbox: PathBuf,
    pub library: PathBuf,
    pub current_ticket: PathBuf,
}

impl Paths {
    pub fn from_env() -> Result<Self> {
        let state_dir = absolute_env_path("FUNKEY_IROH_STATE_DIR", DEFAULT_STATE_DIR)?;
        let inbox = env::var_os("FUNKEY_IROH_INBOX")
            .map(PathBuf::from)
            .unwrap_or_else(|| state_dir.join("inbox"));
        let bundle_inbox = env::var_os("FUNKEY_IROH_BUNDLE_INBOX")
            .map(PathBuf::from)
            .unwrap_or_else(|| state_dir.join("bundle-inbox"));
        let library = absolute_env_path("FUNKEY_IROH_LIBRARY", DEFAULT_LIBRARY_DIR)?;

        for (name, path) in [
            ("FUNKEY_IROH_INBOX", &inbox),
            ("FUNKEY_IROH_BUNDLE_INBOX", &bundle_inbox),
        ] {
            if !path.is_absolute() {
                bail!("{name} must be absolute: {}", path.display());
            }
        }

        Ok(Self {
            identity: state_dir.join("identity"),
            peers: state_dir.join("peers.tsv"),
            current_ticket: state_dir.join("current-ticket"),
            state_dir,
            inbox,
            bundle_inbox,
            library,
        })
    }

    pub fn ensure(&self) -> Result<()> {
        for path in [
            &self.state_dir,
            &self.inbox,
            &self.bundle_inbox,
            &self.library,
        ] {
            std::fs::create_dir_all(path)
                .with_context(|| format!("create Iroh directory {}", path.display()))?;
        }
        Ok(())
    }
}

pub fn max_save_bytes() -> Result<u64> {
    parse_u64_env("FUNKEY_IROH_MAX_SAVE_BYTES", DEFAULT_MAX_SAVE_BYTES)
}

pub fn max_bundle_bytes() -> Result<u64> {
    parse_u64_env(
        "FUNKEY_IROH_MAX_BUNDLE_BYTES",
        DEFAULT_MAX_BUNDLE_BYTES,
    )
}

pub fn max_bundle_files() -> Result<u64> {
    parse_u64_env(
        "FUNKEY_IROH_MAX_BUNDLE_FILES",
        DEFAULT_MAX_BUNDLE_FILES,
    )
}

pub fn max_incoming_connections() -> Result<usize> {
    let value = parse_u64_env(
        "FUNKEY_IROH_MAX_INCOMING_CONNECTIONS",
        DEFAULT_MAX_INCOMING_CONNECTIONS,
    )?;
    if value > 64 {
        bail!("FUNKEY_IROH_MAX_INCOMING_CONNECTIONS must not exceed 64");
    }
    usize::try_from(value).context("convert incoming connection limit")
}

pub fn handshake_timeout() -> Result<Duration> {
    Ok(Duration::from_secs(parse_u64_env(
        "FUNKEY_IROH_HANDSHAKE_TIMEOUT",
        DEFAULT_HANDSHAKE_TIMEOUT_SECS,
    )?))
}

pub fn online_timeout() -> Result<Duration> {
    Ok(Duration::from_secs(parse_u64_env(
        "FUNKEY_IROH_ONLINE_TIMEOUT",
        DEFAULT_ONLINE_TIMEOUT_SECS,
    )?))
}

fn absolute_env_path(name: &str, default: &str) -> Result<PathBuf> {
    let path = env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default));
    if !path.is_absolute() {
        bail!("{name} must be absolute: {}", path.display());
    }
    Ok(path)
}

fn parse_u64_env(name: &str, default: u64) -> Result<u64> {
    match env::var(name) {
        Ok(value) => {
            let parsed = value
                .parse::<u64>()
                .with_context(|| format!("parse {name}={value:?} as an unsigned integer"))?;
            if parsed == 0 {
                bail!("{name} must be greater than zero");
            }
            Ok(parsed)
        }
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(error).with_context(|| format!("read {name}")),
    }
}

pub fn path_is_under(path: &Path, root: &Path) -> bool {
    path == root || path.starts_with(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_boundary_is_component_aware() {
        assert!(path_is_under(
            Path::new("/mnt/Saves/a.sav"),
            Path::new("/mnt")
        ));
        assert!(!path_is_under(
            Path::new("/mnt-not-really/a.sav"),
            Path::new("/mnt")
        ));
    }
}
