# Firmware footprint and content ownership

FunKey-OS has three storage tiers with different update and durability rules:

| Tier | Purpose | Replaced by a routine `.fwu`? |
|---|---|---:|
| Normal ext4 rootfs | Kernel, runtime libraries, frontends, services, current OPKs | Yes |
| Recovery ext4 rootfs | Repair, flashing, and diagnostics | No |
| `/mnt` FAT32 data partition | ROMs, saves, user configuration, and installable content | No |

A routine firmware update must never delete or recreate `/mnt`. Full SD images may
carry bootstrap content needed to initialize a blank card, but that content does
not belong in every later rootfs update.

## Bootstrap content

The canonical first-boot pack is stored in the source tree at:

```text
content/bootstrap/funkey_files.zip
```

The immutable Buildroot target contains only a valid no-op ZIP placeholder at the
legacy path:

```text
/usr/local/share/funkey_files.zip
```

When a full SD image is packaged, `scripts/package-sdcard-image` copies the
completed normal rootfs, replaces the placeholder in that private copy with the
real pack, verifies the bytes, runs `e2fsck`, and gives only that copy to
`genimage`. The original rootfs remains unchanged and becomes the `.fwu`
payload. This keeps first boot compatible while removing roughly 4 MiB from
every routine updater.

Custom image builders may override `FUNKEY_CONTENT_SEED`. The update rootfs must
continue to contain a valid ZIP placeholder because historical post-install code
uses `unzip -n`; the archive contains only a `./` directory entry, exits
successfully, and creates no content or metadata changes under `/mnt`.

## Automated size reports

Every `package-checksums` run also writes:

```text
images/packages-Firmware-size-<version>.json
images/packages-Firmware-size-<version>.txt
```

The existing Actions artifact glob uploads both files for production and derived
profiles. Reports include artifact sizes, ext4 capacity/used/free values, target
tree totals with hard links counted once, largest files, the bootstrap pack, and
budget warnings or failures.

Thresholds live in `scripts/firmware-size-budget.json`. Warning thresholds make
regressions visible; hard limits fail packaging before an unsafe image ships.

## Rules for optional content work

Future layout, skin, language-font, and metadata packs should follow these rules:

1. Keep one complete, dependable default UI in the normal rootfs.
2. Put optional bytes under a versioned managed directory on `/mnt`.
3. Never overwrite user-owned saves, ROMs, screenshots, or configuration without
   an explicit migration and backup.
4. Make first-boot population and routine content upgrades separate operations.
5. Preserve a recovery path that can reconstruct every managed file.
6. Measure every production, debug, minimal, and networked profile with the same
   report schema.

The next candidates are extra RetroFE layouts, non-default GMenu2X skins,
extended-language font packs, and repair binaries that have no normal-mode
caller and are already available in Recovery.
