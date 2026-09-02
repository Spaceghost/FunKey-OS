# Iroh portable games, progress sharing, and netplay transport

The optional `-iroh` firmware profile adds one statically linked Iroh daemon,
an on-device SDL settings application, portable game bundles, PicoArch
lifecycle hooks, and a generic UDP netplay tunnel. Normal firmware images do
not contain these additions.

The implementation uses one persistent Iroh endpoint identity and three
versioned protocols:

| ALPN | Purpose |
| --- | --- |
| `funkey/saves/1` | One authenticated, BLAKE3-checked progress file |
| `funkey/bundles/1` | A complete portable game directory |
| `funkey/netplay/1` | Unreliable, unordered local-UDP datagram tunnel |

Iroh encrypts and mutually authenticates endpoint connections. Direct
peer-to-peer paths are preferred; a relay can carry the encrypted connection
when direct traversal is unavailable.

## On-device UI

Launch **Iroh Share & Play** from the settings section. The interface supports
the RG Nano key mapping as well as ordinary arrow/Enter/Escape keys.

It provides:

- receiver enable/disable and status;
- pairing-ticket QR display and pairing-file import;
- default-peer selection;
- one-button snapshot and send for the last exited game;
- automatic post-game synchronization;
- received-bundle installation;
- RNDIS, ECM, and NCM USB-network selection;
- SFTP/SSHFS connection details;
- netplay transport/adapter readiness;
- local diagnostics.

No UI remains resident while a game is running. PicoArch runs normally. When
it exits, a tiny wrapper records the game and refreshes any already-adopted
portable bundle after PicoArch has closed its save and state files.

The UI reports whether the generic netplay transport is available. Actual
**Host** and **Join** choices belong on each game's pre-launch screen once that
core has a validated UDP or link-cable adapter. The settings application does
not remain resident during play.

## Persistent state

The writable FAT data partition contains identity and queue state:

```text
/mnt/.funkey-iroh/
  identity
  peers.tsv
  current-ticket
  enabled
  default-peer
  autosync
  library.tsv
  last-game.tsv
  service.log
  lifecycle.log
  inbox/
  bundle-inbox/
```

The user-visible, SFTP-mountable portable library is:

```text
/mnt/FunKey/Shared Games/
```

`identity` is exactly 32 bytes of secret Iroh key material. Preserve it to keep
the same device identity across firmware updates. Anyone with physical access
to the FAT partition can copy that key and impersonate the handheld; the
hardware has no secure element.

## Portable `.funkey` bundles

A portable bundle is a directory, not an opaque archive:

```text
Pokemon_Crystal.funkey/
  manifest.tsv
  manifest.json
  files.tsv
  content/
    Pokemon Crystal.gbc
  picoarch/
    Pokemon Crystal.sav
    Pokemon Crystal.rtc
    Pokemon Crystal.st0
    Pokemon Crystal.st0.bmp
    Pokemon Crystal.cfg
    picoarch.cfg
  retroarch/
    saves/
      Pokemon Crystal.srm
      Pokemon Crystal.rtc
    states/
      Pokemon Crystal.state0
      Pokemon Crystal.state0.bmp
    config/gambatte/
    remaps/gambatte/
    system/
  cores/linux-armv7-musleabihf/
```

The bundle can contain:

- the game content itself;
- referenced disc/playlist members for CUE, M3U, CCD, and GDI sets;
- adjacent IPS, BPS, UPS, XDelta, and SBI sidecars;
- SRAM/battery saves;
- RTC state;
- all PicoArch numbered savestates and screenshots;
- game and core configuration;
- RetroArch-compatible directory names;
- explicitly selected, user-owned system/BIOS files;
- optionally, the exact RG Nano ARMv7 core binary.

The normal default records the core path and SHA-256 but does not copy the core
binary. Set `FUNKEY_IROH_INCLUDE_CORE=1` while adopting a game to include that
platform-specific core under `cores/linux-armv7-musleabihf/`.

Savestates are included, but they are inherently more version-sensitive than
ordinary in-game SRAM. A target should use the same core family and a
compatible core version. The core name and hash in the manifest make an
incompatibility diagnosable rather than mysterious.

System/BIOS files are never swept up silently. Put one absolute source path per
line in:

```text
/mnt/.funkey-iroh/include-system.txt
```

Only those explicitly listed files are copied into `retroarch/system/`.

## Adopt and refresh a game

The UI's **Snapshot last game** action adopts the most recently exited game if
necessary, then refreshes its progress.

The direct command is:

```sh
funkey-iroh-library adopt gbc gambatte \
  "/mnt/Game Boy Color/Pokemon Crystal.gbc"
```

Refresh it later:

```sh
funkey-iroh-library refresh \
  "/mnt/FunKey/Shared Games/gbc/Pokemon_Crystal.funkey"
```

Once adopted, every later PicoArch exit refreshes SRAM, RTC, configuration,
and all numbered states. If automatic synchronization is enabled, the updated
directory is then sent to the selected peer.

## Pair devices

Enable the receiver:

```sh
funkey-iroh-service enable
```

The UI can display the pairing ticket as a QR code. It also writes:

```text
/mnt/FunKey/Pairing/rg-nano.ticket
/mnt/FunKey/Pairing/rg-nano-ticket.pbm
/mnt/FunKey/Pairing/rg-nano.endpoint-id
```

To import without typing, place another device's ticket in:

```text
/mnt/FunKey/Pairing/inbox/pocket.ticket
```

Then choose **Import pairing files**. The filename becomes the local peer name.

The direct equivalent is:

```sh
funkey-iroh peer add pocket 'endpoint...'
funkey-iroh peer list
```

Incoming saves, bundles, and multiplayer sessions are paired-only by default.

## Send a complete game

Select a default peer in the UI and use **Send last game**, or run:

```sh
funkey-iroh-library send \
  "/mnt/FunKey/Shared Games/gbc/Pokemon_Crystal.funkey" \
  pocket
```

The bundle protocol:

1. enumerates only regular files and rejects symlinks;
2. rejects absolute paths, `..`, backslashes, duplicates, and reserved paths;
3. hashes every file with BLAKE3;
4. hashes the ordered file manifest;
5. sends bounded metadata and exact byte counts;
6. writes into a private staging directory;
7. verifies every file and the complete manifest;
8. fsyncs files before an atomic directory rename.

An identical destination is skipped. A divergent destination is retained as a
separate `.conflict-...funkey` directory. No remote bundle overwrites an
existing game or progress tree.

Default limits are 4,096 files and 16 GiB total per bundle. Both are
configurable in `/etc/default/funkey-iroh`.

## Install a received bundle

Received bundles remain quarantined under:

```text
/mnt/.funkey-iroh/bundle-inbox/PEER/
```

Choose **Install received** in the UI or run:

```sh
funkey-iroh-library install \
  "/mnt/.funkey-iroh/bundle-inbox/pocket/Pokemon_Crystal.funkey"
```

The RG Nano installer maps the system to its normal content directory and
copies PicoArch progress into the selected core's data directory. Existing
different files are preserved with an `.incoming-TIME` suffix; the installer
does not silently overwrite them.

For a generic RetroArch target, use the ready-made `content`, `retroarch/saves`,
`retroarch/states`, `retroarch/config`, `retroarch/remaps`, and
`retroarch/system` directories directly. A future iOS/iPadOS client can consume
`manifest.json` without inventing another format.

## SFTP and SSHFS

Enabling Iroh also enables the existing USB-network marker. The firmware
already contains Dropbear SSH and its SFTP server. From a connected host:

```text
Host:     192.168.137.2
User:     root
Password: blank
Library:  /mnt/FunKey/Shared Games
```

The release currently follows FunKey-OS's existing USB-only, blank-root
debug/access model. Do not bridge this interface onto an untrusted network.

USB network modes:

- **RNDIS**: Windows default.
- **ECM**: first Apple/Linux compatibility choice.
- **NCM**: alternate Apple/Linux compatibility choice.

Changing mode requires disconnecting/reconnecting USB or rebooting. ECM/NCM
kernel and gadget support is built in, but direct iPhone/iPad compatibility
remains a physical-device release test rather than a CI assumption.

## Multiplayer UDP tunnel

For an emulator or adapter that already exposes local UDP:

Host:

```sh
funkey-iroh-service netplay host \
  127.0.0.1:55300 \
  127.0.0.1:55301
```

Join:

```sh
funkey-iroh-service netplay join \
  pocket \
  127.0.0.1:55300 \
  127.0.0.1:55301
```

Configure the emulator or adapter to send to the bind port and receive from the
target port. Iroh QUIC datagrams preserve UDP packet boundaries and may be
lost or reordered. Oversized and congestion-dropped packets are counted.

PicoArch does not yet expose one generic link-cable/netplay API for every
bundled core. GB/GBC/GBA serial-link adapters are therefore a separate
emulator patch. The transport, identity, pairing, direct/relay path, and UI
session lifecycle are already reusable.

## Debug firmware and recovery

The `-iroh-usb-debug` images are hardware-validation artifacts, not public
release images. They keep the USB debug path enabled so a host can reach the
handheld after boot.

If candidate userspace does not become usable, power the RG Nano off, then
hold **Fn + Start** while powering it on to enter Recovery. Keep the USB cable
connected and use the persistent USB-debug recovery path to inspect or reflash
the device. Automatic watchdog rollback is planned, but the manual chord is
the initial recovery contract and must remain available as an override.

## Build

Iroh 1.0.3 requires Rust 1.91:

```sh
rustup toolchain install 1.91.0 --profile minimal \
  --target arm-unknown-linux-musleabihf
./scripts/build-iroh-firmware
```

The build creates:

```text
images/FunKey-rootfs-<version>-iroh.fwu
images/FunKey-sdcard-<version>-iroh.img.xz
images/FunKey-rootfs-<version>-iroh-usb-debug.fwu
images/FunKey-sdcard-<version>-iroh-usb-debug.img.xz
```

The combined debug profile is for hardware validation, not public releases.

The build verifies:

- native Rust unit and two-node transport tests;
- native SDL UI self-test;
- ARMv7/musl daemon linkage and size;
- ARM UI compilation;
- rootfs injection and byte-for-byte extraction;
- PicoArch wrapper/original preservation;
- filesystem consistency;
- artifact checksums;
- firmware footprint budgets.

See `docs/release-readiness.md` for the master/tagging gate.
