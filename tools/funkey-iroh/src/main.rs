mod bundle;
mod config;
mod endpoint;
mod identity;
mod netplay;
mod peers;
mod save;
mod server;
mod wire;

use std::{
    collections::VecDeque,
    net::SocketAddr,
    path::PathBuf,
};

use anyhow::{Context, Result, bail};
use peers::PeerBook;
use wire::{BUNDLE_ALPN, NETPLAY_ALPN, SAVE_ALPN};

use crate::config::Paths;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let mut arguments: VecDeque<String> = std::env::args().skip(1).collect();
    let Some(command) = arguments.pop_front() else {
        print_usage();
        return Ok(());
    };

    let paths = Paths::from_env()?;

    match command.as_str() {
        "id" => {
            require_empty(&arguments)?;
            paths.ensure()?;
            let secret = identity::load_or_create(&paths.identity)?;
            println!("{}", secret.public());
        }
        "ticket" => {
            require_empty(&arguments)?;
            paths.ensure()?;
            let secret = identity::load_or_create(&paths.identity)?;
            let endpoint = endpoint::bind(
                secret,
                vec![
                    SAVE_ALPN.to_vec(),
                    BUNDLE_ALPN.to_vec(),
                    NETPLAY_ALPN.to_vec(),
                ],
            )
            .await?;
            let ticket = endpoint::ticket(
                &endpoint,
                &paths.current_ticket,
                config::online_timeout()?,
            )
            .await?;
            println!("{ticket}");
            endpoint.close().await;
        }
        "peer" | "peers" => peer_command(paths, arguments)?,
        "serve" => {
            let allow_unpaired = parse_allow_unpaired(arguments)?;
            server::serve(paths, allow_unpaired).await?;
        }
        "save" => save_command(paths, arguments).await?,
        "bundle" => bundle_command(paths, arguments).await?,
        "netplay" => netplay_command(paths, arguments).await?,
        "version" | "--version" | "-V" => {
            require_empty(&arguments)?;
            println!("funkey-iroh {}", env!("CARGO_PKG_VERSION"));
        }
        "help" | "--help" | "-h" => {
            require_empty(&arguments)?;
            print_usage();
        }
        unknown => {
            print_usage();
            bail!("unknown command {unknown:?}");
        }
    }

    Ok(())
}

fn peer_command(paths: Paths, mut arguments: VecDeque<String>) -> Result<()> {
    paths.ensure()?;
    let book = PeerBook::new(paths.peers);
    let subcommand = required(&mut arguments, "peer subcommand")?;

    match subcommand.as_str() {
        "add" => {
            let name = required(&mut arguments, "peer name")?;
            let ticket = required(&mut arguments, "endpoint ticket")?;
            require_empty(&arguments)?;
            let peer = book.add(&name, &ticket)?;
            println!("{}\t{}", peer.name, peer.ticket.endpoint_addr().id);
        }
        "list" => {
            require_empty(&arguments)?;
            for peer in book.list()? {
                println!(
                    "{}\t{}\t{}",
                    peer.name,
                    peer.ticket.endpoint_addr().id,
                    peer.ticket
                );
            }
        }
        "remove" | "rm" => {
            let name = required(&mut arguments, "peer name")?;
            require_empty(&arguments)?;
            if book.remove(&name)? {
                println!("removed {name}");
            } else {
                bail!("peer {name:?} was not paired");
            }
        }
        unknown => bail!("unknown peer subcommand {unknown:?}"),
    }
    Ok(())
}

async fn save_command(paths: Paths, mut arguments: VecDeque<String>) -> Result<()> {
    let subcommand = required(&mut arguments, "save subcommand")?;
    match subcommand.as_str() {
        "send" | "push" => {
            let peer = required(&mut arguments, "peer name or endpoint ticket")?;
            let system = required(&mut arguments, "system identifier")?;
            let game = required(&mut arguments, "game title")?;
            let source = PathBuf::from(required(&mut arguments, "save file")?);
            require_empty(&arguments)?;
            save::send(paths, &peer, &system, &game, &source).await
        }
        unknown => bail!("unknown save subcommand {unknown:?}"),
    }
}

async fn bundle_command(paths: Paths, mut arguments: VecDeque<String>) -> Result<()> {
    let subcommand = required(&mut arguments, "bundle subcommand")?;
    match subcommand.as_str() {
        "send" | "push" => {
            let peer = required(&mut arguments, "peer name or endpoint ticket")?;
            let source = PathBuf::from(required(
                &mut arguments,
                "portable .funkey bundle directory",
            )?);
            require_empty(&arguments)?;
            bundle::send(paths, &peer, &source).await
        }
        "inspect" | "verify" => {
            let source = PathBuf::from(required(
                &mut arguments,
                "portable .funkey bundle directory",
            )?);
            require_empty(&arguments)?;
            bundle::inspect(&source).await
        }
        unknown => bail!("unknown bundle subcommand {unknown:?}"),
    }
}

async fn netplay_command(paths: Paths, mut arguments: VecDeque<String>) -> Result<()> {
    let subcommand = required(&mut arguments, "netplay subcommand")?;
    match subcommand.as_str() {
        "host" | "listen" => {
            let bind = parse_socket(&required(
                &mut arguments,
                "local UDP bind address",
            )?)?;
            let target = parse_socket(&required(
                &mut arguments,
                "emulator UDP target address",
            )?)?;
            let allow_unpaired = parse_allow_unpaired(arguments)?;
            netplay::host(paths, bind, target, allow_unpaired).await
        }
        "join" | "connect" => {
            let peer = required(&mut arguments, "peer name or endpoint ticket")?;
            let bind = parse_socket(&required(
                &mut arguments,
                "local UDP bind address",
            )?)?;
            let target = parse_socket(&required(
                &mut arguments,
                "emulator UDP target address",
            )?)?;
            require_empty(&arguments)?;
            netplay::join(paths, &peer, bind, target).await
        }
        unknown => bail!("unknown netplay subcommand {unknown:?}"),
    }
}

fn parse_allow_unpaired(mut arguments: VecDeque<String>) -> Result<bool> {
    let mut allow_unpaired = false;
    while let Some(argument) = arguments.pop_front() {
        match argument.as_str() {
            "--allow-unpaired" => allow_unpaired = true,
            unknown => bail!("unknown option {unknown:?}"),
        }
    }
    Ok(allow_unpaired)
}

fn parse_socket(value: &str) -> Result<SocketAddr> {
    value.parse().with_context(|| {
        format!("parse socket address {value:?}; include both address and port")
    })
}

fn required(arguments: &mut VecDeque<String>, name: &str) -> Result<String> {
    arguments
        .pop_front()
        .ok_or_else(|| anyhow::anyhow!("missing {name}"))
}

fn require_empty(arguments: &VecDeque<String>) -> Result<()> {
    if arguments.is_empty() {
        Ok(())
    } else {
        bail!(
            "unexpected trailing argument(s): {}",
            arguments.iter().cloned().collect::<Vec<_>>().join(" ")
        )
    }
}

fn print_usage() {
    println!(
        r#"FunKey portable game sharing and local UDP netplay transport over Iroh.

Usage:
  funkey-iroh id
  funkey-iroh ticket
  funkey-iroh peer add NAME ENDPOINT_TICKET
  funkey-iroh peer list
  funkey-iroh peer remove NAME
  funkey-iroh serve [--allow-unpaired]
  funkey-iroh save send PEER SYSTEM GAME SAVE_FILE
  funkey-iroh bundle inspect BUNDLE.funkey
  funkey-iroh bundle send PEER BUNDLE.funkey
  funkey-iroh netplay host BIND_ADDRESS EMULATOR_TARGET [--allow-unpaired]
  funkey-iroh netplay join PEER BIND_ADDRESS EMULATOR_TARGET

Examples:
  funkey-iroh peer add pocket endpoint...
  funkey-iroh save send pocket gbc "Pokemon Crystal" "/mnt/Saves/Pokemon Crystal.sav"
  funkey-iroh bundle send pocket "/mnt/FunKey/Shared Games/gbc/Pokemon_Crystal.funkey"
  funkey-iroh netplay host 127.0.0.1:55300 127.0.0.1:55301
  funkey-iroh netplay join pocket 127.0.0.1:55300 127.0.0.1:55301

Environment:
  FUNKEY_IROH_STATE_DIR         Persistent state directory
  FUNKEY_IROH_INBOX             Received single-file progress directory
  FUNKEY_IROH_BUNDLE_INBOX      Received portable-bundle directory
  FUNKEY_IROH_LIBRARY           Local portable-game library
  FUNKEY_IROH_MAX_SAVE_BYTES    Maximum single progress file
  FUNKEY_IROH_MAX_BUNDLE_BYTES  Maximum complete portable bundle
  FUNKEY_IROH_MAX_BUNDLE_FILES  Maximum files in a portable bundle
  FUNKEY_IROH_ONLINE_TIMEOUT    Seconds to wait for relay registration

Incoming saves, bundles, and netplay sessions are accepted only from paired
endpoint identities unless --allow-unpaired is explicitly supplied."#
    );
}
