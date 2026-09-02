# Persistent USB debug image

The `usb-debug` profile is a hardware-development image for the RG Nano/FunKey
boot and update path. It keeps both USB functions enabled:

- RNDIS networking with passwordless SSH/SFTP at `root@192.168.137.2`
- the existing removable shared-disk function
- a transition journal for mount, gadget, `usb0`, DHCP, Dropbear, Recovery,
  SWUpdate, partition selection, and the next normal boot

Do not ship this profile as retail firmware. It deliberately exposes a
passwordless root shell to the computer attached by USB.

## Artifacts

A build produces three profiles:

| Profile | Full SD image | Firmware update |
| --- | --- | --- |
| Production | `FunKey-sdcard-<version>.img.xz` | `FunKey-rootfs-<version>.fwu` |
| USB network only | `FunKey-sdcard-<version>-network-only.img.xz` | `FunKey-rootfs-<version>-network-only.fwu` |
| Persistent USB debug | `FunKey-sdcard-<version>-usb-debug.img.xz` | `FunKey-rootfs-<version>-usb-debug.fwu` |

Install the full `-usb-debug.img.xz` image once. That puts the debug tooling in
both the normal rootfs and Recovery. Later `.fwu` updates replace only the
normal rootfs, so use the generated `-usb-debug.fwu` while diagnosing boot
failures; the debug Recovery remains installed even if the candidate rootfs is
bad.

Build all profiles locally with:

```sh
make -j"$(nproc)" zig-all
./scripts/build-usb-variants "$(make -s print-version)"
```

## Device-side commands

```sh
ssh root@192.168.137.2 funkey-debug status
ssh root@192.168.137.2 funkey-debug collect manual
ssh root@192.168.137.2 funkey-debug dump
ssh root@192.168.137.2 funkey-debug history
ssh root@192.168.137.2 funkey-debug hold
ssh root@192.168.137.2 funkey-debug release
ssh root@192.168.137.2 funkey-debug root-status
ssh root@192.168.137.2 funkey-debug boot-recovery
ssh root@192.168.137.2 funkey-debug boot-normal
```

`root-status` is intentionally Recovery-only. It runs a read-only ext filesystem
check, mounts partition 2 read-only, and records the target version, init files,
fstab, USB profile, and filesystem usage.

## Host-side flash loop

The host helper transports firmware over SSH rather than exporting the FAT
partition as mass storage. Keep/eject the USB disk so `/mnt` is local on the
device, then run:

```sh
./scripts/funkey-usb-debug status

./scripts/funkey-usb-debug flash \
  images/FunKey-rootfs-<version>-usb-debug.fwu \
  debug-run-001
```

`flash` performs these steps:

1. Refuses ambiguous or non-debug firmware by default.
2. Uploads the FWU and verifies its byte count and SHA-256 when available.
3. Creates `FunKey/Debug/hold-after-flash`.
4. Validates the complete archive in the normal OS.
5. Reboots into Recovery and applies it.
6. Leaves Recovery running after a successful flash.
7. Runs a read-only check and inspection of the new rootfs.
8. Saves the Recovery journal and root report on the host.

After inspection:

```sh
./scripts/funkey-usb-debug boot debug-run-001
```

Or perform the whole gated loop:

```sh
./scripts/funkey-usb-debug cycle \
  images/FunKey-rootfs-<version>-usb-debug.fwu \
  debug-run-001
```

The output set is:

```text
debug-run-001-recovery.log
debug-run-001-root-status.txt
debug-run-001-normal-status.txt
debug-run-001-normal.log
```

Useful overrides:

```sh
FUNKEY_DEBUG_HOST=192.168.137.2
FUNKEY_DEBUG_TIMEOUT=300
FUNKEY_DEBUG_ALLOW_ANY_FIRMWARE=1
```

The non-debug override is intentionally explicit: a normal FWU removes the
debug tooling from the normal rootfs, although the full debug Recovery remains.

## Persistent journal

The current boot writes to:

```text
/var/log/usb-debug.log
```

Whenever the FAT partition is locally mounted, the log is copied to:

```text
/mnt/FunKey/Debug/sessions/<kernel-boot-id>.log
/mnt/FunKey/Debug/current-session
/mnt/FunKey/Debug/update-state
```

When the FAT partition is exported to the host, it cannot also be written by
the device. The volatile log continues collecting and remains readable over
SSH with `funkey-debug dump`; it is flushed on the next local mount.

The monitor records a compact line whenever state changes rather than dumping
continuously. Boot, mount, flash, failure, hold, and reboot boundaries add a
bounded snapshot of:

- kernel command line, versions, uptime, and recent kernel/syslog messages
- partition table, mounted filesystems, and free space
- configfs gadget binding, UDC state/speed, and mass-storage backing file
- `usb0`, interface addresses/routes, resolver state, and service PIDs
- firmware archive names and sizes; the host transport separately verifies SHA-256 when supported
- the durable update state and hold marker

SWUpdate, `resize2fs`, root mounting, asset extraction, unmounting, archive
deletion, and active-partition selection stream their output into the same
journal while preserving their real exit status.

## GitHub hardware loop

`.github/workflows/usb-hardware-loop.yml` is manual-only and targets a runner
with all of these labels:

```text
self-hosted
linux
funkey-usb-debug
```

Connect the debug-installed RG Nano to that runner by USB. Configure the host
side of its RNDIS adapter as `192.168.137.1/24` once, and ensure the runner has
Bash, OpenSSH, and SHA-256 tools. Trigger **FunKey USB hardware loop** with the
successful build workflow run ID. It downloads that run's firmware artifact,
verifies its generated checksum, performs the gated cycle, and uploads all
journals even when the boot fails.

Only one hardware job runs at a time through the
`funkey-usb-debug-hardware` concurrency group.

## Hard early-boot failures

USB networking is created by userspace. If U-Boot, the kernel, or init fails
before configfs and Dropbear start, no remote command can reach that boot. The
held-Recovery stage still proves that the FWU was written, checks the target
filesystem before first boot, and preserves all Recovery/SWUpdate output.

After such an early failure, power on while holding **Fn + Start** to return to
the debug Recovery, then collect:

```sh
./scripts/funkey-usb-debug logs manual-recovery.log
./scripts/funkey-usb-debug history all-persistent-sessions.log
./scripts/funkey-usb-debug root-status manual-root-status.txt
```

That boundary is also where a later UART or bootloader-watchdog fixture would
extend the loop.
