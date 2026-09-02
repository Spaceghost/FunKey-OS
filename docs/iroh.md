# Iroh save sharing and netplay transport

This integration adds an **optional `-iroh` firmware profile**. The normal
FunKey-OS images are unchanged. The profile injects one statically linked
`funkey-iroh` executable and small service scripts into the completed normal
root filesystem.

## Scope

The first implementation provides two versioned application protocols over one
Iroh identity:

| ALPN | Purpose |
| --- | --- |
| `funkey/saves/1` | Authenticated, hash-checked save transfer |
| `funkey/netplay/1` | Unreliable, unordered local-UDP datagram tunnel |

This is real transport, but it is not a claim that every bundled emulator
already implements multiplayer. An emulator or core must expose a local UDP,
netplay, serial-link, or link-cable interface before the generic tunnel can
carry its packets.

## Build

The profile requires Rust 1.91 because Iroh 1.0.3 requires that compiler.

```sh
rustup toolchain install 1.91.0 --profile minimal \
  --target armv7-unknown-linux-musleabihf
./scripts/build-iroh-firmware
```

The script first builds the ordinary Zig firmware, cross-compiles
`tools/funkey-iroh`, verifies that the result is a static ARM executable,
injects it into a temporary copy of `FunKey/output/images/rootfs.ext2`, runs
`e2fsck`, and packages artifacts whose version ends in `-iroh`.

The original root filesystem is restored even when packaging fails.

Expected artifacts include:

```text
images/FunKey-rootfs-<version>-iroh.fwu
images/FunKey-sdcard-<version>-iroh.img.xz
images/SHA256SUMS-<version>-iroh.txt
images/iroh-<version>-iroh.json
```

## Persistent state

The writable FAT data partition stores all mutable state:

```text
/mnt/.funkey-iroh/
  identity
  peers.tsv
  current-ticket
  enabled
  service.log
  inbox/
```

`identity` is exactly 32 bytes of secret Iroh key material. A corrupt or
truncated identity is never silently replaced. Keeping it preserves the
device's endpoint identity across firmware updates.

The FunKey data partition can be exposed as USB mass storage. Anyone with
physical access to that partition can copy this key and impersonate the device.
There is no secure element on this hardware, so physical access remains outside
the protection offered by Iroh's encrypted transport.

## Enable the receiver

```sh
funkey-iroh-service enable
funkey-iroh-service status
funkey-iroh-service ticket
```

The boot service is opt-in. `enable` creates the persistent marker, enables the
existing USB-network profile by creating `/mnt/usbnet`, and starts a paired-only
save receiver. Reconnect USB or reboot once if `usb0` was not present yet.
`disable` removes the Iroh marker but deliberately leaves USB networking enabled.

The receiver starts after the ordinary network and USB-network services. It
continues running through network changes. Internet-wide reachability requires
the RG Nano's USB network to have routing to the internet, such as host
connection sharing. Local tickets can still connect directly when both peers
can route to each other.

## Pair devices

On each device, obtain its ticket while its receiver is running:

```sh
funkey-iroh-service ticket
```

Add the other endpoint under a short local name:

```sh
funkey-iroh peer add pocket 'endpoint...'
funkey-iroh peer list
```

Peer names may contain ASCII letters, digits, `.`, `_`, and `-`.

Incoming saves and multiplayer connections are rejected unless the remote
endpoint identity appears in `peers.tsv`. The explicit `--allow-unpaired`
option exists for supervised setup and temporary game sessions; it is not used
by the boot service.

## Send a save

The service wrapper pauses the local receiver before creating another endpoint
with the same persistent identity, then restores it afterward:

```sh
funkey-iroh-service send \
  pocket \
  gbc \
  'Pokemon Crystal' \
  '/mnt/Saves/Pokemon Crystal.sav'
```

The sender:

1. verifies that the source is a regular file;
2. checks the configured size ceiling;
3. hashes the complete file with BLAKE3;
4. sends bounded metadata over a bidirectional QUIC stream;
5. waits for receiver preflight approval;
6. streams exactly the declared byte count;
7. waits for durable-storage confirmation.

The receiver writes to a temporary file, verifies the declared BLAKE3 hash,
calls `sync_all`, and then atomically renames the file into its inbox.

Received files are grouped by peer, system, and game:

```text
/mnt/.funkey-iroh/inbox/pocket/gbc/Pokemon_Crystal/Pokemon_Crystal.sav
```

Remote path components are sanitized and cannot escape the inbox.

### Conflicts

An identical existing file returns `SKIP`; it is not retransmitted.

A different existing file is never overwritten. The receiver preserves the
incoming version using a name such as:

```text
Pokemon_Crystal.conflict-1788294000123456789-321-a1b2c3d4e5f6.sav
```

Installation into an emulator's live save directory is deliberately separate
from receipt. Save-directory conventions differ between bundled launchers and
cores, and silently guessing the wrong destination is a dependable way to
destroy progress.

## Multiplayer UDP tunnel

For an emulator or adapter that uses local UDP, select two local ports:

- the **bind address** receives outbound packets from the emulator;
- the **target address** receives inbound packets from Iroh.

Host:

```sh
funkey-iroh-service netplay host \
  127.0.0.1:55300 \
  127.0.0.1:55301
```

The host prints a current endpoint ticket. A paired client can use its saved
peer name:

```sh
funkey-iroh-service netplay join \
  rg-nano-host \
  127.0.0.1:55300 \
  127.0.0.1:55301
```

Configure the emulator or adapter to send its remote traffic to
`127.0.0.1:55300` and listen for incoming traffic on `127.0.0.1:55301`.

The tunnel preserves UDP packet boundaries and uses Iroh QUIC datagrams. Packets
may be lost or reordered, matching UDP semantics. Payloads larger than the
negotiated QUIC datagram limit are dropped and counted rather than fragmented
into latency-amplifying streams.

The tunnel reports initial RTT and bounded drop counters. It has no rollback,
frame synchronization, deterministic emulation, or jitter buffer. Those belong
in the emulator-specific layer.

## Direct CLI

The installed commands are:

```text
funkey-iroh id
funkey-iroh ticket
funkey-iroh peer add NAME TICKET
funkey-iroh peer list
funkey-iroh peer remove NAME
funkey-iroh serve [--allow-unpaired]
funkey-iroh save send PEER SYSTEM GAME FILE
funkey-iroh netplay host BIND TARGET [--allow-unpaired]
funkey-iroh netplay join PEER BIND TARGET
```

Use `funkey-iroh-service send`, `netplay`, or `run` when the background receiver
is enabled. The wrapper prevents two live endpoints from publishing the same
identity at once.

Configuration variables in `/etc/default/funkey-iroh`:

```text
FUNKEY_IROH_STATE_DIR
FUNKEY_IROH_INBOX
FUNKEY_IROH_MAX_SAVE_BYTES
FUNKEY_IROH_ONLINE_TIMEOUT
FUNKEY_IROH_SERVICE_ARGS
```

## Next emulator work

The transport boundary is intentionally narrow. Emulator adapters can be added
without modifying Iroh code:

1. add a PicoArch/libretro local datagram API;
2. bridge Game Boy and Game Boy Color serial-link events;
3. bridge GBA link events where the selected core exposes them;
4. add launcher UI for pairing, inbox conflict selection, hosting, and joining;
5. measure frame/link latency on the physical RG Nano before adding buffering.

Until one of those adapters lands, multiplayer is available only to software
that already speaks UDP or can be wrapped with a small local adapter.
