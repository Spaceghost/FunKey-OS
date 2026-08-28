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

# GMU loads libmpg123 through its decoder plugin; no production component
# invokes mpg123's standalone players or tag-maintenance helpers.
rm -f \
	"${TARGET_DIR}/usr/bin/mpg123" \
	"${TARGET_DIR}/usr/bin/mpg123-id3dump" \
	"${TARGET_DIR}/usr/bin/mpg123-strip" \
	"${TARGET_DIR}/usr/bin/out123"
rm -f \
	"${TARGET_DIR}/usr/lib/libout123.so" \
	"${TARGET_DIR}/usr/lib/libout123.so.0" \
	"${TARGET_DIR}/usr/lib/libout123.so.0.2.2"
rm -rf "${TARGET_DIR}/usr/lib/mpg123"

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

# Keep GLib and GStreamer runtime libraries, but omit their uncalled target-side
# administration, debugging, and build-time helpers from production firmware.
rm -f \
	"${TARGET_DIR}/usr/bin/gapplication" \
	"${TARGET_DIR}/usr/bin/gdbus" \
	"${TARGET_DIR}/usr/bin/gio" \
	"${TARGET_DIR}/usr/bin/gio-querymodules" \
	"${TARGET_DIR}/usr/bin/gresource" \
	"${TARGET_DIR}/usr/bin/gsettings"
rm -rf \
	"${TARGET_DIR}/usr/share/gettext/its" \
	"${TARGET_DIR}/usr/share/glib-2.0/valgrind" \
	"${TARGET_DIR}/usr/share/gstreamer-1.0/gdb"

# libbz2 is required by runtime libraries, but its standalone compressor,
# recovery tool, and shell wrappers are unused. BusyBox already provides the
# two decompression applets needed by tar and interactive archive extraction.
rm -f \
	"${TARGET_DIR}/usr/bin/bunzip2" \
	"${TARGET_DIR}/usr/bin/bzcat" \
	"${TARGET_DIR}/usr/bin/bzcmp" \
	"${TARGET_DIR}/usr/bin/bzdiff" \
	"${TARGET_DIR}/usr/bin/bzegrep" \
	"${TARGET_DIR}/usr/bin/bzfgrep" \
	"${TARGET_DIR}/usr/bin/bzgrep" \
	"${TARGET_DIR}/usr/bin/bzip2" \
	"${TARGET_DIR}/usr/bin/bzip2recover" \
	"${TARGET_DIR}/usr/bin/bzless" \
	"${TARGET_DIR}/usr/bin/bzmore"
ln -s ../../bin/busybox "${TARGET_DIR}/usr/bin/bunzip2"
ln -s ../../bin/busybox "${TARGET_DIR}/usr/bin/bzcat"

# liblzma gives libarchive support for common LZMA-compressed 7z files. Drop
# the redundant XZ command suite and restore the smaller BusyBox front-ends.
rm -f \
	"${TARGET_DIR}/usr/bin/lzma" \
	"${TARGET_DIR}/usr/bin/lzmadec" \
	"${TARGET_DIR}/usr/bin/lzmainfo" \
	"${TARGET_DIR}/usr/bin/unlzma" \
	"${TARGET_DIR}/usr/bin/unxz" \
	"${TARGET_DIR}/usr/bin/xz" \
	"${TARGET_DIR}/usr/bin/xzcat" \
	"${TARGET_DIR}/usr/bin/xzcmp" \
	"${TARGET_DIR}/usr/bin/xzdec" \
	"${TARGET_DIR}/usr/bin/xzdiff" \
	"${TARGET_DIR}/usr/bin/xzegrep" \
	"${TARGET_DIR}/usr/bin/xzfgrep" \
	"${TARGET_DIR}/usr/bin/xzgrep" \
	"${TARGET_DIR}/usr/bin/xzless" \
	"${TARGET_DIR}/usr/bin/xzmore"
for applet in lzma unlzma unxz xz xzcat; do
	ln -s ../../bin/busybox "${TARGET_DIR}/usr/bin/${applet}"
done

# Retain the PCRE and libxml2 C libraries used by GLib and libarchive, while
# dropping their uncalled test/front-end tools and unused C++/POSIX PCRE ABIs.
rm -f \
	"${TARGET_DIR}/usr/bin/pcregrep" \
	"${TARGET_DIR}/usr/bin/pcretest" \
	"${TARGET_DIR}/usr/bin/xmlcatalog" \
	"${TARGET_DIR}/usr/bin/xmllint" \
	"${TARGET_DIR}/usr/lib/libpcrecpp.so" \
	"${TARGET_DIR}/usr/lib/libpcrecpp.so.0" \
	"${TARGET_DIR}/usr/lib/libpcrecpp.so.0.0.2" \
	"${TARGET_DIR}/usr/lib/libpcreposix.so" \
	"${TARGET_DIR}/usr/lib/libpcreposix.so.0" \
	"${TARGET_DIR}/usr/lib/libpcreposix.so.0.0.7"

# These optional ABIs have no ELF consumers or name-based loaders in the
# production rootfs. Keep their parent packages and the libraries in use.
rm -f \
	"${TARGET_DIR}/usr/lib/libatopology.so" \
	"${TARGET_DIR}/usr/lib/libatopology.so.2" \
	"${TARGET_DIR}/usr/lib/libatopology.so.2.0.0" \
	"${TARGET_DIR}/usr/lib/libconfig++.so" \
	"${TARGET_DIR}/usr/lib/libconfig++.so.11" \
	"${TARGET_DIR}/usr/lib/libconfig++.so.11.0.2" \
	"${TARGET_DIR}/usr/lib/libgstallocators-1.0.so" \
	"${TARGET_DIR}/usr/lib/libgstallocators-1.0.so.0" \
	"${TARGET_DIR}/usr/lib/libgstallocators-1.0.so.0.1802.0" \
	"${TARGET_DIR}/usr/lib/libgstcontroller-1.0.so" \
	"${TARGET_DIR}/usr/lib/libgstcontroller-1.0.so.0" \
	"${TARGET_DIR}/usr/lib/libgstcontroller-1.0.so.0.1802.0" \
	"${TARGET_DIR}/usr/lib/libgstnet-1.0.so" \
	"${TARGET_DIR}/usr/lib/libgstnet-1.0.so.0" \
	"${TARGET_DIR}/usr/lib/libgstnet-1.0.so.0.1802.0" \
	"${TARGET_DIR}/usr/lib/libgthread-2.0.so" \
	"${TARGET_DIR}/usr/lib/libgthread-2.0.so.0" \
	"${TARGET_DIR}/usr/lib/libgthread-2.0.so.0.6600.3" \
	"${TARGET_DIR}/usr/lib/libss.so" \
	"${TARGET_DIR}/usr/lib/libss.so.2" \
	"${TARGET_DIR}/usr/lib/libss.so.2.0"

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
