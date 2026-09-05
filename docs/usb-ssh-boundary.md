# USB-only SSH boundary

Dropbear is not a LAN service. It listens only on the RG Nano USB gadget
address (`192.168.137.2:22`) and starts only after a fail-closed packet guard
has been installed.

The guard accepts SSH packets only when all of these are true:

- ingress interface is `usb0`;
- destination is the gadget address and TCP port 22;
- source address is the directly attached peer (`192.168.137.1`);
- Ethernet source is the host MAC assigned by the USB gadget.

A normal Ethernet bridge preserves a forwarded sender's source MAC.
Consequently, machines elsewhere on the bridged LAN hit the final drop rule
rather than the direct-peer allow rule.

An explicit peer MAC can be placed in `/mnt/FunKey/.ssh/usb-peer-mac`, one
address per line, for hosts whose bridge deliberately changes its local bridge
MAC. Production images remain public-key-only. Debug images may retain their
laboratory login policy, but the same USB-peer packet boundary applies.

This boundary cannot distinguish the attached computer from software on that
same computer intentionally proxying an SSH byte stream. No protocol running
through a host can defeat a malicious host that relays the protocol itself.
