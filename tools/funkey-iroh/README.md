# funkey-iroh

`funkey-iroh` is the optional FunKey-OS transport for:

- paired, conflict-safe save transfer over `funkey/saves/1`;
- local UDP tunneling over Iroh QUIC datagrams using `funkey/netplay/1`.

It keeps its long-term endpoint identity and peer allowlist under
`FUNKEY_IROH_STATE_DIR` (default `/mnt/.funkey-iroh`).

The normal FunKey-OS firmware does not include this binary. Build the isolated
firmware variant from the repository root:

```sh
./scripts/build-iroh-firmware
```

See [`../../docs/iroh.md`](../../docs/iroh.md) for operation, security,
protocol behavior, and emulator-integration limits.
