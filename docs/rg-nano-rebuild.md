# RG Nano rebuild from upstream (September 2026)

`rg-nano-rebuild` is a fresh line started from DrUm78's `rg_nano` branch
(commit `384286a5`, the RG Nano port of FunKey-OS 2.3.0) with every commit of
this fork's `rg-nano-next` and `rc/v2.4.0-rc.1` lines cherry-picked on top in
their original order (`git cherry-pick -x`, so each commit names its origin).

Two things were deliberately left behind:

- The `Merge upstream master into the RG Nano release line` merge. Upstream
  already carries every `master` change on `rg_nano`; the merge only swapped in
  master's `meta.db`, the two power OPKs, the `buildroot` pin (SDL fork URL)
  and, critically, re-added the 66 MB `freeware_games.zip` that the content
  split had removed. That zip alone pushed the CI firmware to 110 MB and broke
  the 64 MB update budget.
- The fifteen `chore: stage portable Iroh update N/15` payload chunks. Their
  content was materialised into `tools/funkey-iroh` by a later commit that is
  included; the intermediate blobs are not.

## What had to be fixed to build

| Stage | Result |
| --- | --- |
| Pristine upstream `rg_nano` in the pinned Ubuntu 20.04 container | Fails in the `gmu` package: it hardcodes `/opt/FunKey-sdk` and host `mksquashfs`. |
| + build/optimisation stack (checkpoint 1) | Builds; GMU uses the Buildroot toolchain aliases. |
| + everything through the RC hardening (all changes) | Production, network-only and USB-debug profiles build; size budget passes (47 MB `.fwu`, 46 MB `.img.xz`). |
| Iroh profiles | Never built before: rustc's self-contained musl runtime collided with zig cc's `crt1.o`, and debugfs 1.45 silently dropped every injected file. Fixed in `scripts/build-iroh-variant`. |

Additional changes made during the rebuild:

- `scripts/build-in-container` and `docker/focal-ci/Dockerfile.rust`: run any
  target in the pinned image with Podman or Docker, caches kept on the host.
- `make features` / `scripts/funkey-features`: interactive selection of
  outputs, toolchain and on-device applications.
- The version string no longer reports `-dirty` for the Buildroot patches
  that `prepare-buildroot` applies to the submodule.

## Verification performed

- Host-side test scripts (`test-firmware-update`, `test-usb-network-hotplug`,
  `test-usb-menu-info`, `test-usb-debug`, `test-usb-ssh-guard`,
  `test-iroh-ui`) and the native Rust unit tests pass in the container.
- `scripts/test-firmware-packaging` passes against the built images.
- All five profiles (production, `-network-only`, `-usb-debug`, `-iroh`,
  `-iroh-usb-debug`) build from one clean Zig build, with budget PASS and
  SHA-256 manifests.

Not performed: booting any of these images on an RG Nano. The images are
flashable (partition layout and `.fwu` archive structure verified), but until
one has been written to a card and booted, treat them as untested on hardware.

## Flashing

SD card (replaces everything on the card):

```sh
xz -dc images/FunKey-sdcard-<version>.img.xz | sudo dd of=/dev/sdX bs=4M status=progress conv=fsync
```

USB update from a running RG Nano: mount the console over USB, copy
`images/FunKey-rootfs-<version>.fwu` to the top level of the shared partition,
eject; the console installs it on the next unmount.
