use std::{
    env,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail};

pub const DEFAULT_STATE_DIR: &str = "/mnt/.funkey-iroh";
pub const DEFAULT_MAX_SAVE_BYTES: u64 = 64 * 1024 * 1024;
pub const DEFAULT_ONLINE_TIMEOUT_SECS: u64 = 8;

#[derive(Clone, Debug)]
pub struct Paths {
    pub state_dir: PathBuf,
    pub identity: PathBuf,
    pub peers: PathBuf,
    pub inbox: PathBuf,
    pub current_ticket: PathBuf,
}

impl Paths {
    pub fn from_env() -> Result<Self> {
        let state_dir = env::var_os("FUNKEY_IROH_STATE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_STATE_DIR));

        if !state_dir.is_absolute() {
            bail!(
                "FUNKEY_IROH_STATE_DIR must be absolute: {}",
                state_dir.display()
            );
        }

        let inbox = env::var_os("FUNKEY_IROH_INBOX")
            .map(PathBuf::from)
            .unwrap_or_else(|| state_dir.join("inbox"));

        if !inbox.is_absolute() {
            bail!("FUNKEY_IROH_INBOX must be absolute: {}", inbox.display());
        }

        Ok(Self {
            identity: state_dir.join("identity"),
            peers: state_dir.join("peers.tsv"),
            current_ticket: state_dir.join("current-ticket"),
            state_dir,
            inbox,
        })
    }

    pub fn ensure(&self) -> Result<()> {
        std::fs::create_dir_all(&self.state_dir).with_context(|| {
            format!("create Iroh state directory {}", self.state_dir.display())
        })?;
        std::fs::create_dir_all(&self.inbox)
            .with_context(|| format!("create save inbox {}", self.inbox.display()))?;
        Ok(())
    }
}

pub fn max_save_bytes() -> Result<u64> {
    parse_u64_env("FUNKEY_IROH_MAX_SAVE_BYTES", DEFAULT_MAX_SAVE_BYTES)
}

pub fn online_timeout() -> Result<Duration> {
    Ok(Duration::from_secs(parse_u64_env(
        "FUNKEY_IROH_ONLINE_TIMEOUT",
        DEFAULT_ONLINE_TIMEOUT_SECS,
    )?))
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
        assert!(path_is_under(Path::new("/mnt/Saves/a.sav"), Path::new("/mnt")));
        assert!(!path_is_under(
            Path::new("/mnt-not-really/a.sav"),
            Path::new("/mnt")
        ));
    }
}
