use std::{fs, path::Path, time::Duration};

use anyhow::{Context, Result};
use iroh::{Endpoint, SecretKey, endpoint::presets};
use iroh_tickets::endpoint::EndpointTicket;
use tokio::time::timeout;

pub async fn bind(secret_key: SecretKey, alpns: Vec<Vec<u8>>) -> Result<Endpoint> {
    let endpoint = Endpoint::builder(presets::N0)
        .secret_key(secret_key)
        .alpns(alpns)
        .bind()
        .await
        .context("bind Iroh endpoint")?;
    Ok(endpoint)
}

pub async fn ticket(
    endpoint: &Endpoint,
    current_ticket_path: &Path,
    online_timeout: Duration,
) -> Result<EndpointTicket> {
    if timeout(online_timeout, endpoint.online()).await.is_err() {
        eprintln!(
            "funkey-iroh: relay was not reachable within {}s; ticket still contains available local/address-lookup information",
            online_timeout.as_secs()
        );
    }

    let ticket = EndpointTicket::new(endpoint.addr());
    write_atomic(current_ticket_path, ticket.to_string().as_bytes())
        .with_context(|| format!("record current ticket {}", current_ticket_path.display()))?;
    Ok(ticket)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create ticket directory {}", parent.display()))?;
    }
    let temporary = path.with_extension(format!("tmp.{}", std::process::id()));
    let result = (|| -> Result<()> {
        fs::write(&temporary, bytes)
            .with_context(|| format!("write temporary ticket {}", temporary.display()))?;
        let file = fs::OpenOptions::new()
            .read(true)
            .open(&temporary)
            .with_context(|| format!("open temporary ticket {}", temporary.display()))?;
        file.sync_all()
            .with_context(|| format!("sync temporary ticket {}", temporary.display()))?;
        fs::rename(&temporary, path).with_context(|| {
            format!(
                "install current ticket {} as {}",
                temporary.display(),
                path.display()
            )
        })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
