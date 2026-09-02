# FunKey Iroh UI

Small SDL 1.2 launcher/settings front-end for `funkey-iroh`.

The target uses the RG Nano/FunKey key mapping already used by the clock app:

- `u` / `d`: move
- `l` / `r`: secondary actions
- `s`: select
- `q`: back

It has four operating modes:

```text
funkey-iroh-ui
funkey-iroh-ui --launch SYSTEM GAME [ROM]
funkey-iroh-ui --session-status
funkey-iroh-ui --message TITLE TEXT
```

The default screen never performs network or filesystem policy itself. It
invokes the bounded helper commands installed beside the Iroh daemon.
