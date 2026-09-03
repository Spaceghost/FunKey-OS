# USB SSH isolation

Dropbear listens only on `192.168.137.2:22`, the RG Nano side of a `/30`
USB-only subnet. Before Dropbear starts, firmware installs a fail-closed IPv4
INPUT chain that accepts SSH only when all of these match:

- ingress interface `usb0`;
- peer address `192.168.137.1`;
- device address `192.168.137.2`;
- the host MAC assigned by the active RNDIS, ECM, or NCM gadget function.

The peer neighbor entry is pinned to the same MAC. A transparent Ethernet
bridge preserves the original source MAC, so another machine on the bridged
network cannot reach the SSH listener. All firmware profiles, including
USB-debug and Recovery, additionally require a public key from
`/mnt/FunKey/.ssh/authorized_keys`.

For first provisioning, place a public key in `authorized_keys` at the root of
the shared partition and reboot. Firmware copies it to the canonical path if
that path is still empty. Private keys never belong on the RG Nano.

This boundary cannot identify packets that the attached host deliberately
re-originates. Host-side NAT, an SSH proxy, MAC spoofing with the assigned host
identity, or a compromised attached host is therefore inside the trust
boundary. Ordinary bridging is not.
