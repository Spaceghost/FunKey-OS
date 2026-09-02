use std::{
    net::SocketAddr,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use bytes::Bytes;
use iroh::endpoint::Connection;
use tokio::{
    net::UdpSocket,
    signal,
};

use crate::{
    config::{Paths, online_timeout},
    endpoint,
    identity,
    peers::PeerBook,
    wire::NETPLAY_ALPN,
};

const UDP_BUFFER_BYTES: usize = 65_535;
const LOG_DROP_MASK: u64 = 0x3f;

fn current_rtt(connection: &Connection) -> Option<Duration> {
    let paths = connection.paths();
    let selected = paths
        .iter()
        .find(|path| path.is_selected())
        .map(|path| path.rtt());
    selected.or_else(|| paths.iter().map(|path| path.rtt()).min())
}

pub async fn host(
    paths: Paths,
    bind_address: SocketAddr,
    target_address: SocketAddr,
    allow_unpaired: bool,
) -> Result<()> {
    paths.ensure()?;
    let secret = identity::load_or_create(&paths.identity)?;
    let endpoint = endpoint::bind(secret, vec![NETPLAY_ALPN.to_vec()]).await?;
    let ticket = endpoint::ticket(&endpoint, &paths.current_ticket, online_timeout()?).await?;
    println!("{ticket}");
    eprintln!(
        "funkey-iroh: waiting for netplay peer as {} on local UDP {} -> {}",
        endpoint.id(),
        bind_address,
        target_address
    );

    let peer_book = PeerBook::new(paths.peers.clone());
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
                        eprintln!("funkey-iroh: netplay connection failed during handshake: {error:#}");
                        continue;
                    }
                };
                let remote_id = connection.remote_id();
                if !allow_unpaired && !peer_book.is_allowed(&remote_id)? {
                    eprintln!("funkey-iroh: refusing unpaired netplay endpoint {remote_id}");
                    connection.close(1u32.into(), b"unpaired endpoint");
                    continue;
                }

                if let Some(rtt) = current_rtt(&connection) {
                    eprintln!(
                        "funkey-iroh: netplay peer {remote_id} connected; RTT={}ms",
                        rtt.as_millis()
                    );
                } else {
                    eprintln!(
                        "funkey-iroh: netplay peer {remote_id} connected; RTT unavailable"
                    );
                }
                bridge(connection, bind_address, target_address).await?;
                break;
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

pub async fn join(
    paths: Paths,
    peer_name_or_ticket: &str,
    bind_address: SocketAddr,
    target_address: SocketAddr,
) -> Result<()> {
    paths.ensure()?;
    let peer_book = PeerBook::new(paths.peers.clone());
    let ticket = peer_book.resolve(peer_name_or_ticket)?;
    let remote_id = ticket.endpoint_addr().id.clone();

    let secret = identity::load_or_create(&paths.identity)?;
    let endpoint = endpoint::bind(secret, Vec::new()).await?;
    let connection = endpoint
        .connect(ticket.endpoint_addr().clone(), NETPLAY_ALPN)
        .await
        .with_context(|| format!("connect to netplay peer {remote_id}"))?;

    if let Some(rtt) = current_rtt(&connection) {
        eprintln!(
            "funkey-iroh: connected to netplay peer {remote_id}; RTT={}ms; local UDP {} -> {}",
            rtt.as_millis(),
            bind_address,
            target_address
        );
    } else {
        eprintln!(
            "funkey-iroh: connected to netplay peer {remote_id}; RTT unavailable; local UDP {} -> {}",
            bind_address,
            target_address
        );
    }
    bridge(connection, bind_address, target_address).await?;
    endpoint.close().await;
    Ok(())
}

async fn bridge(
    connection: Connection,
    bind_address: SocketAddr,
    target_address: SocketAddr,
) -> Result<()> {
    let udp = UdpSocket::bind(bind_address)
        .await
        .with_context(|| format!("bind local netplay UDP socket {bind_address}"))?;
    let maximum = connection
        .max_datagram_size()
        .ok_or_else(|| anyhow::anyhow!("peer did not negotiate QUIC datagram support"))?;
    if maximum < 256 {
        bail!("negotiated QUIC datagram size {maximum} is too small for netplay");
    }

    eprintln!(
        "funkey-iroh: forwarding UDP packets up to {maximum} bytes; larger packets are dropped"
    );

    let mut buffer = vec![0u8; UDP_BUFFER_BYTES];
    let mut oversize_drops = 0u64;
    let mut congestion_drops = 0u64;
    let shutdown = signal::ctrl_c();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            local = udp.recv_from(&mut buffer) => {
                let (length, source) =
                    local.with_context(|| format!("receive local UDP on {bind_address}"))?;
                if length > maximum {
                    oversize_drops += 1;
                    if oversize_drops == 1 || oversize_drops & LOG_DROP_MASK == 0 {
                        eprintln!(
                            "funkey-iroh: dropped {oversize_drops} oversized local packet(s); latest was {length} bytes from {source}, limit {maximum}"
                        );
                    }
                    continue;
                }

                if let Err(error) =
                    connection.send_datagram(Bytes::copy_from_slice(&buffer[..length]))
                {
                    congestion_drops += 1;
                    if congestion_drops == 1 || congestion_drops & LOG_DROP_MASK == 0 {
                        eprintln!(
                            "funkey-iroh: dropped {congestion_drops} congested datagram(s): {error}"
                        );
                    }
                }
            }
            remote = connection.read_datagram() => {
                let payload = remote.context("read Iroh netplay datagram")?;
                udp.send_to(&payload, target_address)
                    .await
                    .with_context(|| format!("send netplay UDP to {target_address}"))?;
            }
            result = &mut shutdown => {
                result.context("wait for shutdown signal")?;
                break;
            }
        }
    }

    eprintln!(
        "funkey-iroh: netplay stopped; oversized_drops={oversize_drops} congestion_drops={congestion_drops}"
    );
    connection.close(0u32.into(), b"netplay stopped");
    tokio::time::sleep(Duration::from_millis(100)).await;
    Ok(())
}
