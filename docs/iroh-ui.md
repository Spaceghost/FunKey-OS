# Iroh launcher UI and save lifecycle

The optional `-iroh` firmware profile includes a small SDL settings/launcher UI.
The networking endpoint remains headless; the UI appears only for configuration,
before a game, or when the user explicitly opens session status.

## What appears on the RG Nano

A new **Iroh** entry is installed in Settings. It provides:

- receiver enable/disable and health;
- a QR pairing screen plus USB-friendly `.pair` cards;
- paired-device removal;
- per-peer automatic post-game save synchronization;
- conflict-safe received-save review;
- baseline-gated automatic receive;
- queue/session diagnostics;
- multiplayer adapter readiness.

The UI uses the normal FunKey controls:

| Control | Action |
| --- | --- |
| Up / Down | Move |
| Select | Activate or toggle |
| Left / Right | Secondary action where shown |
| Back / Power | Return |

The endpoint daemon does not draw during gameplay.

## Pairing without typing a ticket

Open **Iroh → Pair this device**. The device:

1. starts or reuses its persistent Iroh identity;
2. writes a pairing card to
   `/mnt/FunKey/Iroh/Pairing/<device-name>.pair`;
3. displays the current endpoint ticket as a QR code.

A companion application can scan the QR. For two handhelds without cameras,
copy each `.pair` card into the other device's `FunKey/Iroh/Pairing` directory,
then choose **Import pairing cards**.

The default device name is derived from the endpoint identity. To rename it
without an on-screen keyboard, place a single validated name in:

```text
/mnt/FunKey/Iroh/device-name.txt
```

then choose **Device name → Import device-name.txt**.

## Save synchronization

In **Peers & automatic sync**, select a paired device to toggle post-game sync.

Bundled PicoArch OPKs are rewritten only in the `-iroh` firmware variant so
their existing launch command is prefixed with:

```text
funkey-iroh-launch --system SYSTEM -- ORIGINAL_COMMAND ...
```

The wrapper does not replace the emulator. It:

1. optionally shows the pre-game Solo / Host / Join chooser;
2. starts a configured Iroh multiplayer adapter when selected;
3. runs the original launcher command unchanged;
4. waits for the emulator to close;
5. finds save/state files changed during that game;
6. registers their real local paths;
7. queues one content-hash-qualified transfer per enabled peer;
8. immediately returns control to the launcher.

A background worker drains the queue. Offline peers therefore do not trap the
player behind a connection timeout after every game. Before transmitting, the
worker hashes the current file again and updates stale queue entries rather
than sending a different version under old metadata.

### Received files

Incoming saves first land under:

```text
/mnt/.funkey-iroh/inbox/PEER/SYSTEM/GAME/
```

The UI never guesses a destination. Once that game has been launched locally,
the post-game hook knows its actual save path and the inbox shows
**Install safely**.

Manual install always:

1. validates that the inbox source and target remain under the data partition;
2. copies the current local save into a timestamped backup directory;
3. stages the incoming file beside the target;
4. syncs it;
5. atomically renames it into place;
6. archives the received copy.

**Archive incoming** keeps the file but leaves the live emulator save alone.

### Safe automatic receive

The optional **Safe automatic receive** switch is intentionally not
last-writer-wins. Each synchronized peer has a baseline hash for each save.

An incoming version is installed automatically only when:

- that peer is enabled for synchronization;
- a local target has already been learned;
- the current local file still matches the peer baseline.

If local play changed the save independently, the incoming file stays in the
inbox for review. This turns divergence into a visible conflict instead of a
lost evening.

## Pre-game multiplayer UI

When **Ask before each game** is enabled, launching a wrapped PicoArch title
shows:

```text
Play solo
Host multiplayer
Join <paired device>
Cancel
```

Host and Join appear only when the selected system has a real adapter entry.
The registry format is tab-separated:

```text
SYSTEM    LOCAL_BIND    EMULATOR_TARGET    LABEL
```

Firmware defaults live at:

```text
/etc/funkey-iroh/adapters.tsv
```

User overrides live at:

```text
/mnt/.funkey-iroh/adapters.tsv
```

The user file wins by system identifier.

No bundled PicoArch core currently exposes a stable generic UDP or link-cable
socket, so the firmware registry intentionally ships empty. Until an emulator
adapter is implemented, the launcher displays **Multiplayer adapter
unavailable** rather than offering a decorative button that cannot connect.

Once an adapter exists, the wrapper starts the Iroh datagram bridge before the
emulator and stops it when the game exits. Session state is available through:

```sh
funkey-iroh-ui --session-status
funkey-iroh-session status
```

A future pause-menu hook can invoke that status view; normal multiplayer does
not require an interactive UI mid-game.

## Services and persistent state

The `-iroh` profile installs:

```text
/etc/init.d/S44funkey-iroh
/etc/init.d/S45funkey-iroh-sync
/usr/bin/funkey-iroh-ui
/usr/bin/funkey-iroh-launch
/usr/bin/funkey-iroh-pairing
/usr/bin/funkey-iroh-inbox
/usr/bin/funkey-iroh-outbox
/usr/bin/funkey-iroh-postgame
/usr/bin/funkey-iroh-sync-peer
/usr/bin/funkey-iroh-sync-worker
/usr/bin/funkey-iroh-session
```

Persistent state remains on the writable partition:

```text
/mnt/.funkey-iroh/
  identity
  peers.tsv
  sync-peers
  launch-prompt
  auto-apply
  current-ticket
  inbox/
  outbox/
  archive/
  backups/
  baselines/
  games/
```

Foreground save sends and netplay sessions reserve the device identity with a
bounded lock. The receiver and background sync worker stand down while that
exclusive endpoint is active, then resume afterward.

## Build and verification

```sh
./scripts/test-iroh-ui
./scripts/build-iroh-firmware
```

The build:

- compiles the Iroh daemon with pinned Rust 1.91;
- compiles the SDL UI for ARMv7 with the existing Zig-backed target compiler;
- creates the Settings OPK;
- rewrites completed PicoArch OPKs inside a temporary rootfs copy;
- injects service/helper files;
- reads injected files back for byte comparison;
- verifies every rewritten OPK hash;
- runs `e2fsck`;
- restores the original completed rootfs even on failure;
- packages only the derived `-iroh` image/update artifacts.

The UI OPK and wrapped PicoArch OPKs are copied from immutable rootfs storage
onto the data partition at boot when their content changes. This also upgrades
existing installations where first-boot provisioning has already completed.

## Physical validation checklist

Before merging into `rg-nano-next`:

- [ ] Settings → Iroh appears in both supported launchers.
- [ ] QR and `.pair` export survive reboot.
- [ ] Pairing-card import rejects malformed and self cards.
- [ ] Enabling Iroh survives reboot and USB-network reconnect.
- [ ] A changed save queues without delaying return to the launcher.
- [ ] An offline peer leaves a retryable outbox job.
- [ ] Manual install produces a byte-identical backup.
- [ ] Safe auto-receive applies a clean baseline.
- [ ] Divergent local progress remains in the inbox.
- [ ] Solo launches preserve the original emulator command and exit code.
- [ ] Unsupported systems never offer Host or Join.
- [ ] A test UDP adapter exposes Host / Join and tears down cleanly.
- [ ] CPU, memory, and battery cost are acceptable on physical RG Nano.
