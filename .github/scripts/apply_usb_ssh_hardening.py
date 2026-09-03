#!/usr/bin/env python3
from __future__ import annotations

import os
import stat
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def replace_once(path: str, old: str, new: str) -> None:
    target = ROOT / path
    text = target.read_text()
    if new in text:
        return
    if old not in text:
        raise SystemExit(f"anchor not found in {path}: {old!r}")
    target.write_text(text.replace(old, new, 1))


def write(path: str, content: str, executable: bool = False) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content)
    mode = target.stat().st_mode
    if executable:
        target.chmod(mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    else:
        target.chmod(mode & ~(stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH))


def symlink(path: str, destination: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.is_symlink() or target.exists():
        target.unlink()
    target.symlink_to(destination)


NETFILTER = """CONFIG_NETFILTER=y
CONFIG_NETFILTER_ADVANCED=y
CONFIG_NETFILTER_XTABLES=y
CONFIG_NETFILTER_XT_MATCH_MAC=y
CONFIG_IP_NF_IPTABLES=y
CONFIG_IP_NF_FILTER=y
"""

for config in (
    "FunKey/board/funkey/linux.config",
    "Recovery/board/funkey/linux.config",
):
    replace_once(config, "CONFIG_INET=y\n", "CONFIG_INET=y\n" + NETFILTER)

for config in (
    "FunKey/configs/funkey_defconfig",
    "Recovery/configs/recovery_defconfig",
):
    replace_once(
        config,
        "BR2_PACKAGE_DROPBEAR=y\n",
        "BR2_PACKAGE_DROPBEAR=y\nBR2_PACKAGE_IPTABLES=y\n",
    )

for config in (
    "FunKey/board/funkey/rootfs-overlay/etc/dhcpcd.conf",
    "Recovery/board/funkey/rootfs-overlay/etc/dhcpcd.conf",
):
    replace_once(config, "static ip_address=192.168.137.2/24\n", "static ip_address=192.168.137.2/30\n")

USB_NETWORK = r'''#!/bin/sh

USB_NETWORK_SYSFS_ROOT=${USB_NETWORK_SYSFS_ROOT:-/sys}
USB_NETWORK_GADGET_ROOT=${USB_NETWORK_GADGET_ROOT:-/sys/kernel/config/usb_gadget/FunKey}
USB_NETWORK_INTERFACE=${USB_NETWORK_INTERFACE:-usb0}
USB_NETWORK_DEVICE_ADDRESS=${USB_NETWORK_DEVICE_ADDRESS:-192.168.137.2}
USB_NETWORK_HOST_ADDRESS=${USB_NETWORK_HOST_ADDRESS:-192.168.137.1}
USB_NETWORK_SSH_PORT=${USB_NETWORK_SSH_PORT:-22}
USB_NETWORK_SSH_CHAIN=${USB_NETWORK_SSH_CHAIN:-FUNKEY_USB_SSH}
USB_NETWORK_IP=${USB_NETWORK_IP:-ip}
USB_NETWORK_IPTABLES=${USB_NETWORK_IPTABLES:-iptables}

usb_network_command_exists()
{
    case "$1" in
        */*) [ -x "$1" ] ;;
        *) command -v "$1" >/dev/null 2>&1 ;;
    esac
}

usb_network_is_enabled()
{
    [ -e "$USB_NETWORK_SYSFS_ROOT/class/net/$USB_NETWORK_INTERFACE" ]
}

# A powered cable is not enough: the USB device controller reaches
# "configured" only after a data host has enumerated the gadget. This
# controller leaves that state stale after disconnect, but resets its speed.
usb_data_is_configured()
{
    local usb_speed usb_speed_path usb_state usb_state_path usb_udc

    [ -r "$USB_NETWORK_GADGET_ROOT/UDC" ] || return 1
    usb_udc=
    IFS= read -r usb_udc < "$USB_NETWORK_GADGET_ROOT/UDC" || return 1
    [ -n "$usb_udc" ] || return 1
    usb_state_path="$USB_NETWORK_SYSFS_ROOT/class/udc/$usb_udc/state"
    [ -r "$usb_state_path" ] || return 1
    usb_state=
    IFS= read -r usb_state < "$usb_state_path" || return 1
    [ "$usb_state" = configured ] || return 1

    usb_speed_path="$USB_NETWORK_SYSFS_ROOT/class/udc/$usb_udc/current_speed"
    [ -r "$usb_speed_path" ] || return 1
    usb_speed=
    IFS= read -r usb_speed < "$usb_speed_path" || return 1
    case "$usb_speed" in
        low-speed|full-speed|high-speed|wireless|super-speed|super-speed-plus)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

usb_network_is_ready()
{
    usb_network_is_enabled && usb_data_is_configured
}

usb_network_host_mac()
{
    local host_addr mac

    for host_addr in "$USB_NETWORK_GADGET_ROOT"/functions/*.usb0/host_addr; do
        [ -r "$host_addr" ] || continue
        mac=
        IFS= read -r mac < "$host_addr" || continue
        if printf '%s\n' "$mac" |
           grep -Eq '^[[:xdigit:]]{2}(:[[:xdigit:]]{2}){5}$'; then
            printf '%s\n' "$mac" | tr 'A-F' 'a-f'
            return 0
        fi
    done
    return 1
}

usb_network_has_device_address()
{
    usb_network_command_exists "$USB_NETWORK_IP" || return 1
    "$USB_NETWORK_IP" -4 addr show dev "$USB_NETWORK_INTERFACE" 2>/dev/null |
        grep -Eq "[[:space:]]inet[[:space:]]+$USB_NETWORK_DEVICE_ADDRESS/"
}

usb_network_wait_for_device_address()
{
    local attempt limit

    limit=${1:-15}
    attempt=0
    while ! usb_network_has_device_address; do
        attempt=$((attempt + 1))
        [ "$attempt" -lt "$limit" ] || return 1
        sleep 1
    done
}

usb_network_pin_host_neighbor()
{
    local host_mac

    host_mac=$1
    usb_network_command_exists "$USB_NETWORK_IP" || return 1
    "$USB_NETWORK_IP" neigh del "$USB_NETWORK_HOST_ADDRESS" \
        dev "$USB_NETWORK_INTERFACE" 2>/dev/null || :
    "$USB_NETWORK_IP" neigh add "$USB_NETWORK_HOST_ADDRESS" \
        lladdr "$host_mac" nud permanent dev "$USB_NETWORK_INTERFACE"
}

# Bind identity at both layers. A transparent L2 bridge preserves source MAC,
# so bridged peers keep their own MAC and hit the final DROP. The direct host
# is the only endpoint assigned the gadget host MAC and the only usable peer
# address in the /30.
usb_network_install_ssh_guard()
{
    local host_mac

    case "$USB_NETWORK_SSH_PORT" in
        ''|*[!0-9]*) return 1 ;;
    esac
    [ "$USB_NETWORK_SSH_PORT" -ge 1 ] &&
        [ "$USB_NETWORK_SSH_PORT" -le 65535 ] || return 1
    usb_network_command_exists "$USB_NETWORK_IPTABLES" || return 1
    host_mac=$(usb_network_host_mac) || return 1
    usb_network_wait_for_device_address 15 || return 1
    usb_network_pin_host_neighbor "$host_mac" || return 1

    while "$USB_NETWORK_IPTABLES" -D INPUT -p tcp \
        --dport "$USB_NETWORK_SSH_PORT" \
        -j "$USB_NETWORK_SSH_CHAIN" 2>/dev/null; do
        :
    done

    "$USB_NETWORK_IPTABLES" -F "$USB_NETWORK_SSH_CHAIN" 2>/dev/null ||
        "$USB_NETWORK_IPTABLES" -N "$USB_NETWORK_SSH_CHAIN" || return 1

    # Build the custom chain fail-closed before exposing it from INPUT.
    "$USB_NETWORK_IPTABLES" -A "$USB_NETWORK_SSH_CHAIN" -j DROP || return 1
    "$USB_NETWORK_IPTABLES" -I "$USB_NETWORK_SSH_CHAIN" 1 \
        -i "$USB_NETWORK_INTERFACE" \
        -s "$USB_NETWORK_HOST_ADDRESS" \
        -d "$USB_NETWORK_DEVICE_ADDRESS" \
        -p tcp --dport "$USB_NETWORK_SSH_PORT" \
        -m mac --mac-source "$host_mac" \
        -j ACCEPT || return 1
    "$USB_NETWORK_IPTABLES" -I INPUT 1 -p tcp \
        --dport "$USB_NETWORK_SSH_PORT" \
        -j "$USB_NETWORK_SSH_CHAIN" || return 1
}
'''

for path in (
    "FunKey/board/funkey/rootfs-overlay/usr/local/lib/usb_network",
    "Recovery/board/funkey/rootfs-overlay/usr/local/lib/usb_network",
):
    write(path, USB_NETWORK, executable=True)

DROPBEAR_INIT = r'''#!/bin/sh
# Start Dropbear only after the direct USB host is pinned and the bridge-safe
# source-MAC/IP firewall gate has been installed.

SELF=$(basename "$0")
. /usr/local/lib/usb_network

USB_DEBUG_LIB=${USB_DEBUG_LIB:-/usr/local/lib/usb_debug}
if [ -r "$USB_DEBUG_LIB" ]; then
    . "$USB_DEBUG_LIB"
fi
command -v usb_debug_enabled >/dev/null 2>&1 || usb_debug_enabled() { return 1; }
command -v usb_debug_log >/dev/null 2>&1 || usb_debug_log() { :; }
command -v usb_debug_run >/dev/null 2>&1 ||
    usb_debug_run() { shift; "$@"; }

test -r /etc/default/dropbear && . /etc/default/dropbear
DROPBEAR_ARGS=${DROPBEAR_ARGS:-"-s -g"}
DROPBEAR_BIND_ADDRESS=${DROPBEAR_BIND_ADDRESS:-$USB_NETWORK_DEVICE_ADDRESS}
DROPBEAR_PORT=${DROPBEAR_PORT:-$USB_NETWORK_SSH_PORT}

run_dropbear_daemon()
{
    dropbear_component=$1
    shift
    if usb_debug_enabled; then
        usb_debug_run "$dropbear_component" "$@"
    else
        "$@"
    fi
}

dropbear_arguments_are_safe()
{
    local argument

    for argument in $DROPBEAR_ARGS; do
        case "$argument" in
            -p|-p*)
                echo "dropbear: -p is managed by the USB isolation layer" >&2
                return 1
                ;;
        esac
    done
}

prepare_authorized_keys()
{
    # The shared partition is the only persistent writable filesystem. A key
    # may be provisioned as /authorized_keys from the USB mass-storage side;
    # after one boot its canonical location is FunKey/.ssh/authorized_keys.
    mkdir -p /mnt/FunKey/.ssh
    if [ -r /mnt/authorized_keys ] &&
       [ ! -s /mnt/FunKey/.ssh/authorized_keys ]; then
        cp /mnt/authorized_keys /mnt/FunKey/.ssh/authorized_keys
    fi
}

start()
{
    if ! usb_network_is_ready; then
        usb_debug_log dropbear "start skipped because usb0 is not configured"
        return 0
    fi
    if ! dropbear_arguments_are_safe; then
        usb_debug_log dropbear "refusing unsafe listen arguments"
        return 1
    fi

    USB_NETWORK_DEVICE_ADDRESS=$DROPBEAR_BIND_ADDRESS
    USB_NETWORK_SSH_PORT=$DROPBEAR_PORT
    export USB_NETWORK_DEVICE_ADDRESS USB_NETWORK_SSH_PORT
    if ! usb_network_install_ssh_guard; then
        echo "dropbear: USB host isolation failed; refusing to start" >&2
        usb_debug_log dropbear "host isolation failed; daemon not started"
        return 1
    fi

    prepare_authorized_keys
    DROPBEAR_RUNTIME_ARGS="$DROPBEAR_ARGS -R -p $DROPBEAR_BIND_ADDRESS:$DROPBEAR_PORT"

    # If /etc/dropbear points at /var/run/dropbear, create its volatile target.
    if [ -L /etc/dropbear ] &&
       [ "$(readlink /etc/dropbear)" = "/var/run/dropbear" ]; then
        mkdir -p /var/run/dropbear
    fi

    printf "Starting dropbear sshd: "
    umask 077
    run_dropbear_daemon dropbear-daemon-start \
        start-stop-daemon -S -q -o -p /var/run/dropbear.pid \
        --exec /usr/sbin/dropbear -- $DROPBEAR_RUNTIME_ARGS
    daemon_status=$?
    [ "$daemon_status" -eq 0 ] && echo "OK" || echo "FAIL"
    usb_debug_log dropbear \
        "start status=$daemon_status bind=$DROPBEAR_BIND_ADDRESS:$DROPBEAR_PORT"
    return "$daemon_status"
}

stop()
{
    printf "Stopping dropbear sshd: "
    run_dropbear_daemon dropbear-daemon-stop \
        start-stop-daemon -K -q -o -p /var/run/dropbear.pid
    daemon_status=$?
    [ "$daemon_status" -eq 0 ] && echo "OK" || echo "FAIL"
    usb_debug_log dropbear "stop status=$daemon_status"
    # Leave the INPUT gate installed: stopped remains closed.
    return "$daemon_status"
}

case "${1:-}" in
    start) start ;;
    stop) stop ;;
    restart|reload)
        stop
        start
        ;;
    *)
        echo "Usage: $0 {start|stop|restart}" >&2
        exit 1
        ;;
esac

exit $?
'''

write(
    "FunKey/board/funkey/rootfs-overlay/etc/init.d/S50dropbear",
    DROPBEAR_INIT,
    executable=True,
)
write(
    "Recovery/board/funkey/rootfs-overlay/etc/init.d/S42dropbear",
    DROPBEAR_INIT,
    executable=True,
)

DROPBEAR_DEFAULT = r'''#!/bin/sh

# Authentication and reachability are both fail-closed: public keys only,
# bound to the device side of the point-to-point USB network.
DROPBEAR_ARGS="-s -g"
DROPBEAR_BIND_ADDRESS=192.168.137.2
DROPBEAR_PORT=22
'''
for path in (
    "FunKey/board/funkey/rootfs-overlay/etc/default/dropbear",
    "Recovery/board/funkey/rootfs-overlay/etc/default/dropbear",
):
    write(path, DROPBEAR_DEFAULT)

# Dropbear resolves root's authorized_keys below /root. Keep that interface but
# put the actual key on the writable shared partition in every image, including
# persistent Recovery and USB-debug builds.
symlink("FunKey/board/funkey/rootfs-overlay/root/.ssh", "/mnt/FunKey/.ssh")
symlink("Recovery/board/funkey/rootfs-overlay/root/.ssh", "/mnt/FunKey/.ssh")

TEST = r'''#!/bin/bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
sys="$tmp/sys"
gadget="$tmp/gadget"
mock="$tmp/mock"
mkdir -p \
    "$sys/class/net/usb0" \
    "$sys/class/udc/test-udc" \
    "$gadget/functions/rndis.usb0" \
    "$mock"
printf '%s\n' test-udc > "$gadget/UDC"
printf '%s\n' configured > "$sys/class/udc/test-udc/state"
printf '%s\n' high-speed > "$sys/class/udc/test-udc/current_speed"
printf '%s\n' 12:34:56:78:9A:BC > "$gadget/functions/rndis.usb0/host_addr"

cat > "$mock/ip" <<'EOF'
#!/bin/sh
printf 'ip %s\n' "$*" >> "$MOCK_LOG"
if [ "$1" = -4 ]; then
    printf '5: usb0: <UP> mtu 1500\n    inet 192.168.137.2/30 scope global usb0\n'
    exit 0
fi
if [ "$1" = neigh ] && [ "$2" = del ]; then
    exit 1
fi
exit 0
EOF

cat > "$mock/iptables" <<'EOF'
#!/bin/sh
printf 'iptables %s\n' "$*" >> "$MOCK_LOG"
case "$1" in
    -D|-F) exit 1 ;;
    *) exit 0 ;;
esac
EOF
chmod 0755 "$mock/ip" "$mock/iptables"

export MOCK_LOG="$tmp/commands.log"
export USB_NETWORK_SYSFS_ROOT="$sys"
export USB_NETWORK_GADGET_ROOT="$gadget"
export USB_NETWORK_IP="$mock/ip"
export USB_NETWORK_IPTABLES="$mock/iptables"

# shellcheck disable=SC1090
. "$repo/FunKey/board/funkey/rootfs-overlay/usr/local/lib/usb_network"
usb_network_is_ready
[ "$(usb_network_host_mac)" = 12:34:56:78:9a:bc ]
usb_network_install_ssh_guard

grep -F -- \
    'ip neigh add 192.168.137.1 lladdr 12:34:56:78:9a:bc nud permanent dev usb0' \
    "$MOCK_LOG"
grep -F -- \
    'iptables -I FUNKEY_USB_SSH 1 -i usb0 -s 192.168.137.1 -d 192.168.137.2 -p tcp --dport 22 -m mac --mac-source 12:34:56:78:9a:bc -j ACCEPT' \
    "$MOCK_LOG"
grep -F -- 'iptables -A FUNKEY_USB_SSH -j DROP' "$MOCK_LOG"
grep -F -- \
    'iptables -I INPUT 1 -p tcp --dport 22 -j FUNKEY_USB_SSH' \
    "$MOCK_LOG"

for shell in dash 'busybox sh'; do
    $shell -n "$repo/FunKey/board/funkey/rootfs-overlay/usr/local/lib/usb_network"
    $shell -n "$repo/FunKey/board/funkey/rootfs-overlay/etc/init.d/S50dropbear"
    $shell -n "$repo/Recovery/board/funkey/rootfs-overlay/etc/init.d/S42dropbear"
done

cmp \
    "$repo/FunKey/board/funkey/rootfs-overlay/usr/local/lib/usb_network" \
    "$repo/Recovery/board/funkey/rootfs-overlay/usr/local/lib/usb_network"
grep -qx 'DROPBEAR_ARGS="-s -g"' \
    "$repo/FunKey/board/funkey/rootfs-overlay/etc/default/dropbear"
grep -qx 'DROPBEAR_ARGS="-s -g"' \
    "$repo/Recovery/board/funkey/rootfs-overlay/etc/default/dropbear"
grep -q 'usb_network_install_ssh_guard' \
    "$repo/FunKey/board/funkey/rootfs-overlay/etc/init.d/S50dropbear"
grep -q -- '-p $DROPBEAR_BIND_ADDRESS:$DROPBEAR_PORT' \
    "$repo/FunKey/board/funkey/rootfs-overlay/etc/init.d/S50dropbear"

for config in \
    "$repo/FunKey/configs/funkey_defconfig" \
    "$repo/Recovery/configs/recovery_defconfig"
do
    grep -qx 'BR2_PACKAGE_IPTABLES=y' "$config"
done
for config in \
    "$repo/FunKey/board/funkey/linux.config" \
    "$repo/Recovery/board/funkey/linux.config"
do
    grep -qx 'CONFIG_NETFILTER=y' "$config"
    grep -qx 'CONFIG_NETFILTER_XT_MATCH_MAC=y' "$config"
    grep -qx 'CONFIG_IP_NF_FILTER=y' "$config"
done
for config in \
    "$repo/FunKey/board/funkey/rootfs-overlay/etc/dhcpcd.conf" \
    "$repo/Recovery/board/funkey/rootfs-overlay/etc/dhcpcd.conf"
do
    grep -qx 'static ip_address=192.168.137.2/30' "$config"
done

[ "$(readlink "$repo/FunKey/board/funkey/rootfs-overlay/root/.ssh")" = /mnt/FunKey/.ssh ]
[ "$(readlink "$repo/Recovery/board/funkey/rootfs-overlay/root/.ssh")" = /mnt/FunKey/.ssh ]

printf 'USB SSH isolation tests: PASS\n'
'''
write("scripts/test-usb-ssh-guard", TEST, executable=True)

DOC = r'''# USB SSH isolation

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
'''
write("docs/usb-ssh-isolation.md", DOC)

# Make the full Iroh workflow exercise the permanent guard test as part of the
# exact firmware candidate build.
replace_once(
    ".github/workflows/iroh.yml",
    "          ./scripts/test-iroh-ui\n\n          cargo build",
    "          ./scripts/test-usb-ssh-guard\n          ./scripts/test-iroh-ui\n\n          cargo build",
)

print("USB SSH hardening patch applied")
