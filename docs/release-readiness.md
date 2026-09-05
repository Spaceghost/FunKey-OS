# RG Nano master and release readiness

`rg-nano-next` is the integration branch. `master` is releasable history.
Green compilation alone is not enough to promote an embedded firmware build.

## Required merge order

1. Merge the persistent USB-debug/hardware-loop work into `rg-nano-next`.
2. Rebase or merge the Iroh/UI branch against that integration head.
3. Make both the ordinary FunKey build and the Iroh build green.
4. Build the combined `-iroh-usb-debug` updater.
5. Run **FunKey Iroh hardware loop** on the dedicated RG Nano fixture.
6. Review the uploaded hardware evidence.
7. Merge the Iroh/UI pull request into `rg-nano-next`.
8. Open one integration pull request from `rg-nano-next` to `master`.
9. Require all build checks again on the exact integration merge candidate.
10. Merge to `master`, run a release candidate, and only then create a stable
    tag.

## Automated software gates

All boxes must be green on the exact commit proposed for `master`.

- [ ] Buildroot production firmware builds from a clean checkout.
- [ ] Network-only firmware builds.
- [ ] Persistent USB-debug firmware builds.
- [ ] Iroh release firmware builds.
- [ ] Combined Iroh + USB-debug firmware builds.
- [ ] Rust 1.91 dependency lock is committed.
- [ ] Native Rust unit tests pass.
- [ ] Two local Iroh endpoints transfer a complete bundle.
- [ ] Identical re-send returns `SKIP`.
- [ ] Divergent re-send creates a conflict directory.
- [ ] Absolute paths, traversal, backslashes, symlinks, duplicate paths, file
      count overflow, and byte-count overflow are rejected.
- [ ] SRAM, RTC, numbered savestates, state screenshots, content, and sidecars
      appear in a generated portable bundle.
- [ ] Native SDL UI compiles with warnings as errors and passes dummy-video
      self-test.
- [ ] ARMv7/musl daemon is static and contains no dynamic `NEEDED` entries.
- [ ] ARM UI binary links against only libraries already present in firmware.
- [ ] PicoArch original binary is retained as `picoarch.real`.
- [ ] PicoArch wrapper, service, library helper, UI action helper, UI OPK, and
      daemon extract byte-for-byte from the final rootfs.
- [ ] `e2fsck` passes after each derived-image mutation.
- [ ] Every published artifact verifies against its SHA-256 manifest.
- [ ] Common firmware footprint reports exist for every profile.
- [ ] The named Iroh budget passes without silently weakening production
      limits.
- [ ] No debug firmware is selected as a public release asset.

## Required hardware-loop evidence

The **FunKey Iroh hardware loop** must run on a dedicated self-hosted runner
with an RG Nano already running the persistent USB-debug profile.

The selected run must prove:

- [ ] Firmware upload size and SHA-256 match before reboot.
- [ ] Recovery boots and reports `held-after-flash`.
- [ ] Read-only inspection of the flashed normal rootfs succeeds.
- [ ] Normal userspace returns over USB after Recovery releases it.
- [ ] Persistent boot and update journals are collected.
- [ ] `funkey-iroh`, service, library helper, UI, and lifecycle wrapper exist.
- [ ] On-device UI dummy-video self-test passes.
- [ ] On-device shell helper self-tests pass.
- [ ] SFTP uploads and downloads the same bytes.
- [ ] Host-to-device portable bundle transfer succeeds.
- [ ] Device-to-host portable bundle transfer succeeds.
- [ ] SRAM and at least one numbered savestate survive the round trip.
- [ ] Duplicate transfer is skipped.
- [ ] Divergent transfer is preserved as a conflict.
- [ ] ECM and NCM gadget modules exist.
- [ ] Debug history contains no mount, flash, filesystem, or service failure.

The run ID is mandatory input to the release workflow. Its evidence artifact is
downloaded, checked for required files/failure markers, and embedded in the
release candidate artifact.

## Manual physical gates

These cannot be honestly automated by a single permanently attached fixture.

- [ ] Cold boot from fully powered off.
- [ ] Warm reboot.
- [ ] Close/open or suspend/resume behavior appropriate to the RG Nano.
- [ ] Play at least one GB/GBC title, create SRAM, create two state slots,
      exit, relaunch, and restore both.
- [ ] Receive and install the same game on a clean second target.
- [ ] Validate a disc/playlist content set if PS1 content is in release scope.
- [ ] Confirm a conflicting local save is never overwritten.
- [ ] Confirm recovery entry remains available after a broken normal boot.
- [ ] Windows RNDIS SFTP access.
- [ ] Linux ECM or NCM SFTP access.
- [ ] iPhone USB-C SFTP access with ECM and/or NCM.
- [ ] iPad USB-C SFTP access with ECM and/or NCM.
- [ ] Thirty-minute Iroh transfer/relay soak without thermal, memory, or
      watchdog failure.
- [ ] Battery impact is acceptable with receiver idle and during transfer.
- [ ] A previous stable updater can be reinstalled through Recovery.

## Release cadence

Use semantic versions and make the first integrated release a candidate:

```text
v2.4.0-rc.1
v2.4.0-rc.2       only if fixes are required
v2.4.0
```

Do not retag. A failed candidate gets a new candidate number.

The `Release RG Nano firmware` workflow requires:

- a version;
- a commit/ref already contained by `master`;
- a successful hardware-loop run ID;
- the exact confirmation `RELEASE_RG_NANO`;
- approval through the protected `rg-nano-release` environment.

It rebuilds release assets from the selected `master` commit, reruns native
tests, verifies the chosen hardware evidence, excludes all `usb-debug`
artifacts, creates an annotated tag, and publishes the GitHub release.

## Public assets

Stable releases should contain:

- production update and compressed SD image;
- network-only update and compressed SD image;
- Iroh update and compressed SD image;
- per-profile SHA-256 manifests;
- per-profile firmware footprint reports;
- Iroh build metadata;
- one aggregate checksum manifest;
- release evidence identifying the exact commit and hardware run.

Never publish the persistent USB-debug or combined Iroh USB-debug firmware as a
normal retail asset. Keep those as Actions artifacts for lab recovery and CI.

## Known scope boundary

The release can ship save sharing, savestate sharing, portable game bundles,
SFTP/SSHFS access, UI, and generic UDP-over-Iroh transport before every
emulator has native multiplayer.

Do not claim GB/GBC/GBA link-cable multiplayer until a selected PicoArch core
adapter has passed two-device gameplay tests. The transport can be production
ready while a particular emulator adapter remains experimental.
