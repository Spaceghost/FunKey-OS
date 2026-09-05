# funkey-iroh

`funkey-iroh` is the optional FunKey-OS sidecar for encrypted, paired device
sharing over Iroh:

- conflict-preserving one-file progress transfer (`funkey/saves/1`);
- complete portable game/save/savestate bundles (`funkey/bundles/1`);
- local UDP tunneling for emulator adapters (`funkey/netplay/1`).

The `-iroh` firmware also installs **Iroh Share & Play**, a tiny SDL settings
application, an SFTP-visible library under `/mnt/FunKey/Shared Games`, and a
PicoArch lifecycle wrapper that snapshots adopted games only after the emulator
has closed its progress files.

Mutable identity, pairing, inbox, and queue state live under
`FUNKEY_IROH_STATE_DIR` (default `/mnt/.funkey-iroh`). The ordinary production
firmware does not include these additions.

Build the release and combined USB-debug variants from the repository root:

```sh
./scripts/build-iroh-firmware
```

See [`../../docs/iroh.md`](../../docs/iroh.md) for operation and
[`../../docs/release-readiness.md`](../../docs/release-readiness.md) for the
hardware and release gates.
