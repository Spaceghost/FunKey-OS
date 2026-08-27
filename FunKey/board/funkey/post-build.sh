#!/bin/sh

set -e

# Add local path to init scripts
path_line='export PATH=/sbin:/usr/sbin:/bin:/usr/bin:/usr/local/sbin:/usr/local/bin'
for script in "${TARGET_DIR}/etc/init.d/rcK" "${TARGET_DIR}/etc/init.d/rcS"; do
	if ! grep -qxF "$path_line" "$script"; then
		sed -i "3i$path_line" "$script"
	fi
done

# Remove log daemon init scripts since they are loaded from inittab
rm -f ${TARGET_DIR}/etc/init.d/S01syslogd ${TARGET_DIR}/etc/init.d/S02klogd

# LZO installs standalone demonstration programs that are not used at runtime.
rm -rf "${TARGET_DIR}/usr/libexec/lzo/examples"
rmdir "${TARGET_DIR}/usr/libexec/lzo" 2>/dev/null || true

# The on-device system statistics overlay uses mpstat only. Keep the complete
# sysstat suite in Recovery, but omit unused production collectors and reports.
rm -f \
	"${TARGET_DIR}/usr/bin/cifsiostat" \
	"${TARGET_DIR}/usr/bin/iostat" \
	"${TARGET_DIR}/usr/bin/pidstat" \
	"${TARGET_DIR}/usr/bin/sadf" \
	"${TARGET_DIR}/usr/bin/sar" \
	"${TARGET_DIR}/usr/bin/tapestat" \
	"${TARGET_DIR}/usr/lib64/sa/sa1" \
	"${TARGET_DIR}/usr/lib64/sa/sa2" \
	"${TARGET_DIR}/usr/lib64/sa/sadc" \
	"${TARGET_DIR}/etc/sysconfig/sysstat" \
	"${TARGET_DIR}/etc/sysconfig/sysstat.ioconf"
rmdir "${TARGET_DIR}/usr/lib64/sa" 2>/dev/null || true
rmdir "${TARGET_DIR}/etc/sysconfig" 2>/dev/null || true

# Remove dhcp lib dir and link to /tmp
rm -rf "${TARGET_DIR}/var/lib/dhcp"
ln -s /tmp "${TARGET_DIR}/var/lib/dhcp"

# Remove dhcpcd dir and link to /tmp
mkdir -p "${TARGET_DIR}/var/db"
rm -rf "${TARGET_DIR}/var/db/dhcpcd"
ln -s /tmp "${TARGET_DIR}/var/db/dhcpcd"

# Redirect drobear keys to /tmp
rm -rf "${TARGET_DIR}/etc/dropbear"
ln -s /tmp "${TARGET_DIR}/etc/dropbear"

# Change dropbear init sequence
if [ -e "${TARGET_DIR}/etc/init.d/S50dropbear" ]; then
	mv "${TARGET_DIR}/etc/init.d/S50dropbear" "${TARGET_DIR}/etc/init.d/S42dropbear"
fi

# Store byte-identical static assets only once while preserving every path.
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
python3 "${script_dir}/../../../scripts/deduplicate-rootfs-assets" "${TARGET_DIR}"
