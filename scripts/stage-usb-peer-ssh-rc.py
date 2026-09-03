#!/usr/bin/env python3
"""Stage fail-closed USB-peer-only SSH policy for v2.4.0-rc.1."""

from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def write(path: str, content: str, mode: int = 0o644) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content)
    target.chmod(mode)


def ensure_kconfig(path: Path, symbol: str, value: str = "y") -> None:
    text = path.read_text()
    pattern = re.compile(
        rf"(?m)^(?:{re.escape(symbol)}=.*|# {re.escape(symbol)} is not set)\n?"
    )
    text = pattern.sub("", text).rstrip() + f"\n{symbol}={value}\n"
    path.write_text(text)


config = r'''# USB peer-only SSH policy. These addresses belong to the point-to-point
# USB gadget link, not to Wi-Fi or Ethernet.
FUNKEY_USB_SSH_INTERFACE=usb0
FUNKEY_USB_SSH_DEVICE_IP=192.168.137.2
FUNKEY_USB_SSH_PEER_IP=192.168.137.1
FUNKEY_USB_SSH_PORT=22
FUNKEY_USB_SSH_REFRESH_SECONDS=2
FUNKEY_USB_SSH_AUTHORIZED_KEYS=/mnt/FunKey/.ssh/authorized_keys
'''
write(
    "FunKey/board/funkey/rootfs-overlay/etc/default/funkey-usb-ssh",
    config,
)
write(
    "Recovery/board/funkey/rootfs-overlay/etc/default/funkey-usb-ssh",
    config,
)

helper = r'''#!/bin/sh
# Sourced by S50dropbear. Security invariants here deliberately override
# profile-specific convenience flags, including USB-debug profiles.

FUNKEY_USB_SSH_CONFIG=${FUNKEY_USB_SSH_CONFIG:-/etc/default/funkey-usb-ssh}
[ -r "$FUNKEY_USB_SSH_CONFIG" ] && . "$FUNKEY_USB_SSH_CONFIG"

FUNKEY_USB_SSH_DEVICE_IP=${FUNKEY_USB_SSH_DEVICE_IP:-192.168.137.2}
FUNKEY_USB_SSH_PEER_IP=${FUNKEY_USB_SSH_PEER_IP:-192.168.137.1}
FUNKEY_USB_SSH_PORT=${FUNKEY_USB_SSH_PORT:-22}
FUNKEY_USB_SSH_AUTHORIZED_KEYS=${FUNKEY_USB_SSH_AUTHORIZED_KEYS:-/mnt/FunKey/.ssh/authorized_keys}
FUNKEY_USB_SSH_GUARD=${FUNKEY_USB_SSH_GUARD:-/usr/local/sbin/funkey-usb-ssh-guard}

funkey_usb_ssh_sanitize_args()
{
    old_args=$1
    new_args=
    skip_next=0
    set -f
    # DROPBEAR_ARGS is an administrator-owned shell word list in Buildroot.
    # shellcheck disable=SC2086
    set -- $old_args
    set +f
    for argument do
        if [ "$skip_next" -eq 1 ]; then
            skip_next=0
            continue
        fi
        case "$argument" in
            -B|-s|-g|-j|-k)
                ;;
            -p)
                skip_next=1
                ;;
            -p*)
                ;;
            *)
                new_args="$new_args $argument"
                ;;
        esac
    done
    printf '%s\n' "$new_args -s -g -j -k -p ${FUNKEY_USB_SSH_DEVICE_IP}:${FUNKEY_USB_SSH_PORT}"
}

funkey_usb_ssh_have_authorized_key()
{
    for key_file in \
        "$FUNKEY_USB_SSH_AUTHORIZED_KEYS" \
        /mnt/.ssh/authorized_keys \
        /mnt/authorized_keys \
        /root/.ssh/authorized_keys
    do
        [ -s "$key_file" ] && return 0
    done
    return 1
}

funkey_usb_ssh_prepare()
{
    if ! funkey_usb_ssh_have_authorized_key; then
        echo "dropbear: refusing to start without an authorized key on the USB data partition" >&2
        return 78
    fi
    if [ ! -x "$FUNKEY_USB_SSH_GUARD" ]; then
        echo "dropbear: peer firewall guard is missing" >&2
        return 69
    fi
    "$FUNKEY_USB_SSH_GUARD" start || return $?

    DROPBEAR_ARGS=$(funkey_usb_ssh_sanitize_args "${DROPBEAR_ARGS-}")
    DROPBEAR_OPTS=$(funkey_usb_ssh_sanitize_args "${DROPBEAR_OPTS-${DROPBEAR_ARGS}}")
    DAEMON_ARGS=$(funkey_usb_ssh_sanitize_args "${DAEMON_ARGS-${DROPBEAR_ARGS}}")
    DROPBEAR_PORT="${FUNKEY_USB_SSH_DEVICE_IP}:${FUNKEY_USB_SSH_PORT}"
    DROPBEAR_BIND=$FUNKEY_USB_SSH_DEVICE_IP
    export DROPBEAR_ARGS DROPBEAR_OPTS DAEMON_ARGS DROPBEAR_PORT DROPBEAR_BIND
}

funkey_usb_ssh_guard_stop()
{
    [ -x "$FUNKEY_USB_SSH_GUARD" ] || return 0
    "$FUNKEY_USB_SSH_GUARD" stop
}
'''
write(
    "FunKey/board/funkey/rootfs-overlay/usr/local/lib/funkey-usb-ssh-policy",
    helper,
)
write(
    "Recovery/board/funkey/rootfs-overlay/usr/local/lib/funkey-usb-ssh-policy",
    helper,
)

guard = r'''#!/bin/sh
# Fail-closed ingress filter for Dropbear. A normal L2 bridge preserves the
# original sender MAC, so only the USB host peer can reach TCP/22. Key-only
# authentication remains mandatory because a routing/NAT host necessarily
# collapses downstream identity into its own.

set -u
CONFIG=${FUNKEY_USB_SSH_CONFIG:-/etc/default/funkey-usb-ssh}
[ -r "$CONFIG" ] && . "$CONFIG"

IFACE=${FUNKEY_USB_SSH_INTERFACE:-usb0}
DEVICE_IP=${FUNKEY_USB_SSH_DEVICE_IP:-192.168.137.2}
PEER_IP=${FUNKEY_USB_SSH_PEER_IP:-192.168.137.1}
PORT=${FUNKEY_USB_SSH_PORT:-22}
REFRESH=${FUNKEY_USB_SSH_REFRESH_SECONDS:-2}
IPTABLES=${IPTABLES:-iptables}
IP6TABLES=${IP6TABLES:-ip6tables}
ARP_TABLE=${FUNKEY_USB_SSH_ARP_TABLE:-/proc/net/arp}
PIDFILE=${FUNKEY_USB_SSH_PIDFILE:-/var/run/funkey-usb-ssh-guard.pid}
STATEFILE=${FUNKEY_USB_SSH_STATEFILE:-/var/run/funkey-usb-ssh-peer.mac}
CHAIN=FUNKEY_USB_SSH
CHAIN6=FUNKEY_USB_SSH6

valid_settings()
{
    case "$IFACE" in ''|*[!A-Za-z0-9_.:-]*) return 1 ;; esac
    case "$DEVICE_IP:$PEER_IP" in *[!0-9.:]*) return 1 ;; esac
    case "$PORT" in ''|*[!0-9]*) return 1 ;; esac
    case "$REFRESH" in ''|*[!0-9]*) return 1 ;; esac
    [ "$PORT" -gt 0 ] && [ "$PORT" -le 65535 ] && [ "$REFRESH" -gt 0 ]
}

peer_mac()
{
    [ -r "$ARP_TABLE" ] || return 1
    awk -v wanted_ip="$PEER_IP" -v wanted_dev="$IFACE" '
        $1 == wanted_ip && $6 == wanted_dev && $3 != "0x0" {
            print tolower($4)
            exit
        }
    ' "$ARP_TABLE" | grep -E '^[0-9a-f]{2}(:[0-9a-f]{2}){5}$'
}

ensure_ipv4_chain()
{
    command -v "$IPTABLES" >/dev/null 2>&1 || {
        echo "funkey-usb-ssh-guard: iptables is unavailable; SSH remains closed" >&2
        return 69
    }
    "$IPTABLES" -N "$CHAIN" 2>/dev/null || true
    "$IPTABLES" -C "$CHAIN" -j DROP 2>/dev/null || "$IPTABLES" -A "$CHAIN" -j DROP
    "$IPTABLES" -C INPUT -p tcp --dport "$PORT" -j "$CHAIN" 2>/dev/null ||
        "$IPTABLES" -I INPUT 1 -p tcp --dport "$PORT" -j "$CHAIN"
}

remove_old_allow()
{
    [ -r "$STATEFILE" ] || return 0
    old_mac=
    IFS= read -r old_mac < "$STATEFILE" || old_mac=
    case "$old_mac" in
        [0-9a-f][0-9a-f]:[0-9a-f][0-9a-f]:[0-9a-f][0-9a-f]:[0-9a-f][0-9a-f]:[0-9a-f][0-9a-f]:[0-9a-f][0-9a-f])
            "$IPTABLES" -D "$CHAIN" -i "$IFACE" \
                -s "$PEER_IP" -d "$DEVICE_IP" -p tcp --dport "$PORT" \
                -m mac --mac-source "$old_mac" -j ACCEPT 2>/dev/null || true
            ;;
    esac
    rm -f "$STATEFILE"
}

refresh_ipv4()
{
    ensure_ipv4_chain || return $?
    new_mac=$(peer_mac 2>/dev/null || true)
    old_mac=
    [ -r "$STATEFILE" ] && IFS= read -r old_mac < "$STATEFILE"
    [ "$new_mac" = "$old_mac" ] && [ -n "$new_mac" ] && return 0

    # The terminal DROP remains installed while the allow rule is replaced.
    remove_old_allow
    if [ -n "$new_mac" ]; then
        "$IPTABLES" -I "$CHAIN" 1 -i "$IFACE" \
            -s "$PEER_IP" -d "$DEVICE_IP" -p tcp --dport "$PORT" \
            -m mac --mac-source "$new_mac" -j ACCEPT
        umask 077
        printf '%s\n' "$new_mac" > "$STATEFILE"
    fi
}

ensure_ipv6_closed()
{
    command -v "$IP6TABLES" >/dev/null 2>&1 || return 0
    "$IP6TABLES" -N "$CHAIN6" 2>/dev/null || true
    "$IP6TABLES" -C "$CHAIN6" -j DROP 2>/dev/null || "$IP6TABLES" -A "$CHAIN6" -j DROP
    "$IP6TABLES" -C INPUT -p tcp --dport "$PORT" -j "$CHAIN6" 2>/dev/null ||
        "$IP6TABLES" -I INPUT 1 -p tcp --dport "$PORT" -j "$CHAIN6"
}

running()
{
    [ -r "$PIDFILE" ] || return 1
    pid=
    IFS= read -r pid < "$PIDFILE" || return 1
    case "$pid" in ''|*[!0-9]*) return 1 ;; esac
    kill -0 "$pid" 2>/dev/null
}

monitor()
{
    trap 'rm -f "$PIDFILE"; exit 0' INT TERM HUP EXIT
    while :; do
        ping -c 1 -W 1 -I "$IFACE" "$PEER_IP" >/dev/null 2>&1 || true
        refresh_ipv4 || true
        sleep "$REFRESH"
    done
}

start_guard()
{
    valid_settings || {
        echo "funkey-usb-ssh-guard: invalid configuration; SSH remains closed" >&2
        return 78
    }
    ensure_ipv4_chain || return $?
    ensure_ipv6_closed || return $?
    refresh_ipv4 || return $?
    if ! running; then
        mkdir -p "$(dirname "$PIDFILE")"
        "$0" monitor >/dev/null 2>&1 &
        printf '%s\n' "$!" > "$PIDFILE"
    fi
}

stop_guard()
{
    if running; then
        kill "$pid" 2>/dev/null || true
    fi
    rm -f "$PIDFILE"
    if command -v "$IPTABLES" >/dev/null 2>&1; then
        remove_old_allow
        "$IPTABLES" -D INPUT -p tcp --dport "$PORT" -j "$CHAIN" 2>/dev/null || true
        "$IPTABLES" -F "$CHAIN" 2>/dev/null || true
        "$IPTABLES" -X "$CHAIN" 2>/dev/null || true
    fi
    if command -v "$IP6TABLES" >/dev/null 2>&1; then
        "$IP6TABLES" -D INPUT -p tcp --dport "$PORT" -j "$CHAIN6" 2>/dev/null || true
        "$IP6TABLES" -F "$CHAIN6" 2>/dev/null || true
        "$IP6TABLES" -X "$CHAIN6" 2>/dev/null || true
    fi
}

case "${1:-}" in
    start) start_guard ;;
    stop) stop_guard ;;
    restart) stop_guard; start_guard ;;
    refresh|once) valid_settings && ensure_ipv4_chain && ensure_ipv6_closed && refresh_ipv4 ;;
    monitor) monitor ;;
    status) running ;;
    *) echo "Usage: $0 {start|stop|restart|refresh|status}" >&2; exit 64 ;;
esac
'''
write(
    "FunKey/board/funkey/rootfs-overlay/usr/local/sbin/funkey-usb-ssh-guard",
    guard,
    0o755,
)
write(
    "Recovery/board/funkey/rootfs-overlay/usr/local/sbin/funkey-usb-ssh-guard",
    guard,
    0o755,
)

policy_block = r'''
# BEGIN FUNKEY USB-PEER SSH POLICY
FUNKEY_USB_SSH_POLICY=${FUNKEY_USB_SSH_POLICY:-/usr/local/lib/funkey-usb-ssh-policy}
if [ -r "$FUNKEY_USB_SSH_POLICY" ]; then
    . "$FUNKEY_USB_SSH_POLICY"
    case "${1:-}" in
        start|restart)
            funkey_usb_ssh_prepare || exit $?
            ;;
        stop)
            trap 'funkey_usb_ssh_guard_stop' EXIT
            ;;
    esac
else
    case "${1:-}" in
        start|restart)
            echo "dropbear: USB-peer SSH policy is missing; refusing to start" >&2
            exit 69
            ;;
    esac
fi
# END FUNKEY USB-PEER SSH POLICY
'''

for relative in (
    "FunKey/board/funkey/rootfs-overlay/etc/init.d/S50dropbear",
    "Recovery/board/funkey/rootfs-overlay/etc/init.d/S50dropbear",
):
    path = ROOT / relative
    if not path.exists():
        continue
    text = path.read_text()
    if "BEGIN FUNKEY USB-PEER SSH POLICY" not in text:
        location = text.rfind("\ncase ")
        if location < 0:
            raise SystemExit(f"cannot locate main case statement in {relative}")
        text = text[: location + 1] + policy_block.lstrip("\n") + "\n" + text[location + 1 :]
        path.write_text(text)
        path.chmod(0o755)

# Keep any existing comments/host-key settings, but remove argument settings
# that could open an additional listener or re-enable blank passwords.
for relative in (
    "FunKey/board/funkey/rootfs-overlay/etc/default/dropbear",
    "Recovery/board/funkey/rootfs-overlay/etc/default/dropbear",
):
    path = ROOT / relative
    if not path.exists():
        continue
    lines = [
        line
        for line in path.read_text().splitlines()
        if not re.match(
            r"^(?:DROPBEAR_ARGS|DROPBEAR_OPTS|DAEMON_ARGS|DROPBEAR_PORT|DROPBEAR_BIND)=",
            line,
        )
    ]
    lines.extend(
        [
            "",
            "# Mandatory USB-address, key-only, no-forwarding listener policy.",
            'DROPBEAR_BIND="192.168.137.2"',
            'DROPBEAR_PORT="192.168.137.2:22"',
            'DROPBEAR_ARGS="-s -g -j -k -p 192.168.137.2:22"',
            'DROPBEAR_OPTS="$DROPBEAR_ARGS"',
            'DAEMON_ARGS="$DROPBEAR_ARGS"',
        ]
    )
    path.write_text("\n".join(lines).rstrip() + "\n")

# Userspace firewall and the exact kernel match path are release invariants.
for path in ROOT.rglob("*defconfig"):
    text = path.read_text(errors="ignore")
    if "BR2_PACKAGE_DROPBEAR=y" in text:
        ensure_kconfig(path, "BR2_PACKAGE_IPTABLES")

kernel_symbols = {
    "CONFIG_NETFILTER": "y",
    "CONFIG_NETFILTER_ADVANCED": "y",
    "CONFIG_NETFILTER_XTABLES": "y",
    "CONFIG_NETFILTER_XT_MATCH_MAC": "y",
    "CONFIG_IP_NF_IPTABLES": "y",
    "CONFIG_IP_NF_FILTER": "y",
}
for path in ROOT.rglob("linux.config"):
    for symbol, value in kernel_symbols.items():
        ensure_kconfig(path, symbol, value)

# Future software builds must run the isolation contract before packaging.
for relative in (
    ".github/workflows/build.yml",
    ".github/workflows/iroh.yml",
    ".github/workflows/release.yml",
):
    path = ROOT / relative
    if not path.exists():
        continue
    text = path.read_text()
    if "./scripts/test-usb-ssh-isolation" in text:
        continue
    anchors = (
        "          ./scripts/test-iroh-ui\n",
        "          ./scripts/test-iroh-portability \\\n",
        "          make -j\"$(nproc)\" zig-all\n",
    )
    for anchor in anchors:
        if anchor in text:
            text = text.replace(
                anchor,
                "          ./scripts/test-usb-ssh-isolation\n" + anchor,
                1,
            )
            path.write_text(text)
            break

# Static and adversarial test with stubbed packet-filter commands.
test = r'''#!/bin/bash
set -euo pipefail
repo=$(cd "$(dirname "$0")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

for script in \
  "$repo/FunKey/board/funkey/rootfs-overlay/usr/local/sbin/funkey-usb-ssh-guard" \
  "$repo/FunKey/board/funkey/rootfs-overlay/usr/local/lib/funkey-usb-ssh-policy"
do
  dash -n "$script"
done

grep -q 'BEGIN FUNKEY USB-PEER SSH POLICY' \
  "$repo/FunKey/board/funkey/rootfs-overlay/etc/init.d/S50dropbear"
grep -q -- '-s -g -j -k -p 192.168.137.2:22' \
  "$repo/FunKey/board/funkey/rootfs-overlay/etc/default/dropbear"
grep -q '^BR2_PACKAGE_IPTABLES=y$' "$repo/FunKey/configs/funkey_defconfig"
for symbol in CONFIG_NETFILTER CONFIG_NETFILTER_XTABLES \
  CONFIG_NETFILTER_XT_MATCH_MAC CONFIG_IP_NF_IPTABLES CONFIG_IP_NF_FILTER
do
  grep -q "^${symbol}=y$" "$repo/FunKey/board/funkey/linux.config"
done

cat > "$tmp/iptables" <<'SH'
#!/bin/sh
printf '%s\n' "$*" >> "$IPTABLES_LOG"
case " $* " in *' -C '*) exit 1 ;; esac
exit 0
SH
cp "$tmp/iptables" "$tmp/ip6tables"
chmod +x "$tmp/iptables" "$tmp/ip6tables"
cat > "$tmp/ping" <<'SH'
#!/bin/sh
exit 1
SH
chmod +x "$tmp/ping"
cat > "$tmp/config" <<EOF
FUNKEY_USB_SSH_INTERFACE=usb0
FUNKEY_USB_SSH_DEVICE_IP=192.168.137.2
FUNKEY_USB_SSH_PEER_IP=192.168.137.1
FUNKEY_USB_SSH_PORT=22
FUNKEY_USB_SSH_REFRESH_SECONDS=2
EOF
cat > "$tmp/arp" <<'EOF'
IP address       HW type     Flags       HW address            Mask     Device
192.168.137.1    0x1         0x2         02:11:22:33:44:55     *        usb0
EOF

export PATH="$tmp:$PATH"
export IPTABLES_LOG="$tmp/iptables.log"
export FUNKEY_USB_SSH_CONFIG="$tmp/config"
export FUNKEY_USB_SSH_ARP_TABLE="$tmp/arp"
export FUNKEY_USB_SSH_PIDFILE="$tmp/guard.pid"
export FUNKEY_USB_SSH_STATEFILE="$tmp/peer.mac"
guard="$repo/FunKey/board/funkey/rootfs-overlay/usr/local/sbin/funkey-usb-ssh-guard"
"$guard" once

grep -F -- '-I FUNKEY_USB_SSH 1 -i usb0 -s 192.168.137.1 -d 192.168.137.2 -p tcp --dport 22 -m mac --mac-source 02:11:22:33:44:55 -j ACCEPT' "$IPTABLES_LOG"
grep -F -- '-A FUNKEY_USB_SSH -j DROP' "$IPTABLES_LOG"
grep -F -- '-I INPUT 1 -p tcp --dport 22 -j FUNKEY_USB_SSH' "$IPTABLES_LOG"
grep -F -- '-A FUNKEY_USB_SSH6 -j DROP' "$IPTABLES_LOG"

: > "$IPTABLES_LOG"
: > "$tmp/arp"
rm -f "$tmp/peer.mac"
"$guard" once
! grep -q -- '--mac-source' "$IPTABLES_LOG"
grep -F -- '-A FUNKEY_USB_SSH -j DROP' "$IPTABLES_LOG"

# Profile flags may try to add another listener or blank-password mode; the
# sourced policy must delete those and append one canonical listener.
mkdir -p "$tmp/mnt/FunKey/.ssh"
printf 'ssh-ed25519 AAAATEST rc-test\n' > "$tmp/mnt/FunKey/.ssh/authorized_keys"
cat > "$tmp/noop-guard" <<'SH'
#!/bin/sh
printf '%s\n' "$*" >> "$GUARD_LOG"
SH
chmod +x "$tmp/noop-guard"
export GUARD_LOG="$tmp/guard.log"
export FUNKEY_USB_SSH_GUARD="$tmp/noop-guard"
export FUNKEY_USB_SSH_AUTHORIZED_KEYS="$tmp/mnt/FunKey/.ssh/authorized_keys"
# shellcheck disable=SC1090
source "$repo/FunKey/board/funkey/rootfs-overlay/usr/local/lib/funkey-usb-ssh-policy"
DROPBEAR_ARGS='-R -B -p 0.0.0.0:22 -p [::]:22 -K 30'
funkey_usb_ssh_prepare
[[ "$DROPBEAR_ARGS" == *'-s -g -j -k -p 192.168.137.2:22'* ]]
[[ "$DROPBEAR_ARGS" != *' -B '* ]]
[[ "$DROPBEAR_ARGS" != *'0.0.0.0'* ]]
[[ "$DROPBEAR_ARGS" != *'[::]'* ]]
[[ $(grep -o -- ' -p ' <<<"$DROPBEAR_ARGS" | wc -l) -eq 1 ]]
grep -qx start "$GUARD_LOG"

rm -f "$FUNKEY_USB_SSH_AUTHORIZED_KEYS"
if funkey_usb_ssh_prepare 2>/dev/null; then
  echo 'policy accepted a keyless SSH service' >&2
  exit 1
fi

printf 'USB peer-only SSH isolation: PASS\n'
'''
write("scripts/test-usb-ssh-isolation", test, 0o755)

# Release documentation must say what is and is not distinguishable after a
# host deliberately routes/NATs traffic.
doc = r'''# USB peer-only SSH boundary

Dropbear listens only on `192.168.137.2:22`, disables password login and both
SSH forwarding directions, and refuses to start without an authorized key.
An INPUT-chain guard permits TCP/22 only from `192.168.137.1` on `usb0` and
the MAC address learned for that directly attached USB peer. All other IPv4
and all IPv6 TCP/22 traffic is dropped.

A layer-2 bridge preserves downstream source MAC addresses, so bridged LAN
clients cannot reach SSH. A host that deliberately routes, NATs, or proxies a
connection replaces downstream identity with its own; no endpoint can infer
that hidden origin. Key-only authentication remains the cryptographic boundary
in that case. USB-debug images therefore do not re-enable blank passwords.
'''
write("docs/usb-peer-ssh.md", doc)

print("staged USB peer-only SSH policy")
