use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::{Context, Result, bail};
use iroh::EndpointId;
use iroh_tickets::endpoint::EndpointTicket;

#[derive(Clone, Debug)]
pub struct Peer {
    pub name: String,
    pub ticket: EndpointTicket,
}

#[derive(Clone, Debug)]
pub struct PeerBook {
    path: PathBuf,
}

impl PeerBook {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn list(&self) -> Result<Vec<Peer>> {
        let mut peers = Vec::new();
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(peers),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("read peer book {}", self.path.display()));
            }
        };

        for (index, raw_line) in text.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (name, ticket_text) = line.split_once('\t').ok_or_else(|| {
                anyhow::anyhow!(
                    "{}:{}: expected NAME<TAB>ENDPOINT_TICKET",
                    self.path.display(),
                    index + 1
                )
            })?;
            validate_name(name).with_context(|| {
                format!("{}:{}: invalid peer name", self.path.display(), index + 1)
            })?;
            let ticket = EndpointTicket::from_str(ticket_text.trim()).with_context(|| {
                format!(
                    "{}:{}: invalid endpoint ticket for peer {name:?}",
                    self.path.display(),
                    index + 1
                )
            })?;
            peers.push(Peer {
                name: name.to_owned(),
                ticket,
            });
        }
        peers.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(peers)
    }

    pub fn add(&self, name: &str, ticket_text: &str) -> Result<Peer> {
        validate_name(name)?;
        let ticket = EndpointTicket::from_str(ticket_text)
            .with_context(|| format!("parse endpoint ticket for peer {name:?}"))?;

        let mut peers: BTreeMap<String, EndpointTicket> = self
            .list()?
            .into_iter()
            .map(|peer| (peer.name, peer.ticket))
            .collect();

        let endpoint_id = &ticket.endpoint_addr().id;
        if let Some(existing) = peers.iter().find(|(existing_name, existing_ticket)| {
            existing_name.as_str() != name && &existing_ticket.endpoint_addr().id == endpoint_id
        }) {
            bail!(
                "endpoint {endpoint_id} is already paired as {:?}",
                existing.0
            );
        }

        peers.insert(name.to_owned(), ticket.clone());
        self.write(&peers)?;

        Ok(Peer {
            name: name.to_owned(),
            ticket,
        })
    }

    pub fn remove(&self, name: &str) -> Result<bool> {
        validate_name(name)?;
        let mut peers: BTreeMap<String, EndpointTicket> = self
            .list()?
            .into_iter()
            .map(|peer| (peer.name, peer.ticket))
            .collect();
        let removed = peers.remove(name).is_some();
        if removed {
            self.write(&peers)?;
        }
        Ok(removed)
    }

    pub fn resolve(&self, name_or_ticket: &str) -> Result<EndpointTicket> {
        if let Ok(ticket) = EndpointTicket::from_str(name_or_ticket) {
            return Ok(ticket);
        }

        self.list()?
            .into_iter()
            .find(|peer| peer.name == name_or_ticket)
            .map(|peer| peer.ticket)
            .ok_or_else(|| anyhow::anyhow!("unknown peer {name_or_ticket:?}"))
    }

    pub fn is_allowed(&self, endpoint_id: &EndpointId) -> Result<bool> {
        Ok(self
            .list()?
            .iter()
            .any(|peer| &peer.ticket.endpoint_addr().id == endpoint_id))
    }

    pub fn name_for(&self, endpoint_id: &EndpointId) -> Result<Option<String>> {
        Ok(self
            .list()?
            .into_iter()
            .find(|peer| &peer.ticket.endpoint_addr().id == endpoint_id)
            .map(|peer| peer.name))
    }

    fn write(&self, peers: &BTreeMap<String, EndpointTicket>) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create peer directory {}", parent.display()))?;
        }

        let temporary = temporary_path(&self.path);
        let result = (|| -> Result<()> {
            let mut file = fs::File::create(&temporary)
                .with_context(|| format!("create temporary peer book {}", temporary.display()))?;
            writeln!(
                file,
                "# FunKey Iroh peers. Format: NAME<TAB>ENDPOINT_TICKET"
            )?;
            for (name, ticket) in peers {
                writeln!(file, "{name}\t{ticket}")?;
            }
            file.sync_all()
                .with_context(|| format!("sync temporary peer book {}", temporary.display()))?;
            fs::rename(&temporary, &self.path).with_context(|| {
                format!(
                    "replace peer book {} with {}",
                    self.path.display(),
                    temporary.display()
                )
            })?;
            Ok(())
        })();

        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 48 {
        bail!("peer name must contain between 1 and 48 bytes");
    }
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        bail!("peer name may contain only ASCII letters, digits, '.', '-' and '_'");
    }
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    path.with_extension(format!("tmp.{}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_names_are_shell_and_tsv_safe() {
        for valid in ["pocket", "rg-nano-2", "desk_1", "parent.device"] {
            validate_name(valid).unwrap();
        }
        for invalid in ["", "../bad", "has space", "has\ttab", "slash/name"] {
            assert!(validate_name(invalid).is_err(), "{invalid:?}");
        }
    }
}
