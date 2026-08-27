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
