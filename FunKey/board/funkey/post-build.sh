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
# If either file is missing or a package changes one copy, leave both alone.
deduplicate_file() {
	canonical="${TARGET_DIR}$1"
	duplicate="${TARGET_DIR}$2"
	if [ -f "$canonical" ] && [ -f "$duplicate" ] && cmp -s "$canonical" "$duplicate"; then
		rm -f "$duplicate"
		ln "$canonical" "$duplicate"
	fi
}

while read -r canonical duplicate; do
	deduplicate_file "$canonical" "$duplicate"
done <<'EOF'
/usr/share/fonts/droid/DroidSansFallback.ttf /usr/bin/commander/DroidSansFallback.ttf
/usr/local/share/ProdResources/FreeSansBold.ttf /usr/games/menu_resources/FreeSansBold.ttf
/usr/local/share/ProdResources/OpenSans-ExtraBoldItalic.ttf /usr/games/menu_resources/OpenSans-ExtraBoldItalic.ttf
/usr/local/share/ProdResources/OpenSans-ExtraBold.ttf /usr/games/menu_resources/OpenSans-ExtraBold.ttf
/usr/local/share/ProdResources/OpenSans-SemiboldItalic.ttf /usr/games/menu_resources/OpenSans-SemiboldItalic.ttf
/usr/local/share/ProdResources/OpenSans-BoldItalic.ttf /usr/games/menu_resources/OpenSans-BoldItalic.ttf
/usr/local/share/ProdResources/OpenSans-LightItalic.ttf /usr/games/menu_resources/OpenSans-LightItalic.ttf
/usr/local/share/ProdResources/OpenSans-Bold.ttf /usr/games/menu_resources/OpenSans-Bold.ttf
/usr/local/share/ProdResources/arial.ttf /usr/games/menu_resources/arial.ttf
/usr/local/share/ProdResources/courbd.ttf /usr/games/menu_resources/courbd.ttf
/usr/local/share/ProdResources/OpenSans-Italic.ttf /usr/games/menu_resources/OpenSans-Italic.ttf
/usr/local/share/ProdResources/OpenSans-Semibold.ttf /usr/games/menu_resources/OpenSans-Semibold.ttf
/usr/local/share/ProdResources/OpenSans-Light.ttf /usr/games/menu_resources/OpenSans-Light.ttf
/usr/local/share/ProdResources/OpenSans-Regular.ttf /usr/games/menu_resources/OpenSans-Regular.ttf
/usr/games/menu_resources/OpenSans-Bold.ttf /usr/games/layouts/FunKey/OpenSans-Bold.ttf
/usr/games/menu_resources/OpenSans-Bold.ttf /usr/games/layouts/FunKeyRed/OpenSans-Bold.ttf
/usr/games/menu_resources/OpenSans-Bold.ttf /usr/games/layouts/FunKeyYellow/OpenSans-Bold.ttf
/usr/games/menu_resources/OpenSans-Regular.ttf /usr/games/layouts/FunKey/OpenSans-Regular.ttf
/usr/games/menu_resources/OpenSans-Regular.ttf /usr/games/layouts/FunKeyRed/OpenSans-Regular.ttf
/usr/games/menu_resources/OpenSans-Regular.ttf /usr/games/layouts/FunKeyYellow/OpenSans-Regular.ttf
/usr/games/layouts/Artbook-sml/Gilroy-Bold.ttf /usr/games/layouts/Daijismol/Gilroy-Bold.ttf
/usr/games/layouts/Artbook-sml/Gilroy-Bold.ttf /usr/games/layouts/DarkUI/Gilroy-Bold.ttf
/usr/games/layouts/Artbook-sml/Gilroy-Bold.ttf /usr/games/layouts/GameBoy/Gilroy-Bold.ttf
/usr/games/layouts/Artbook-sml/Gilroy-Bold.ttf /usr/games/layouts/RetroRoomCovers/Gilroy-Bold.ttf
/usr/games/layouts/Classic/Roboto-Bold.ttf /usr/games/layouts/Flat/Roboto-Bold.ttf
/usr/games/layouts/Classic/Roboto-Bold.ttf /usr/games/layouts/FunKey/Roboto-Bold.ttf
/usr/games/layouts/Classic/Roboto-Bold.ttf /usr/games/layouts/Superlopez/Roboto-Bold.ttf
/usr/games/layouts/Classic/Roboto-Bold.ttf /usr/games/layouts/TFT/Roboto-Bold.ttf
/usr/games/layouts/FunKey/sounds/select.wav /usr/games/layouts/Classic/sounds/select.wav
/usr/games/layouts/FunKey/sounds/select.wav /usr/games/layouts/Flat/sounds/select.wav
/usr/games/layouts/FunKey/sounds/select.wav /usr/games/layouts/Superlopez/sounds/select.wav
/usr/games/layouts/FunKey/sounds/load.wav /usr/games/layouts/Classic/sounds/load.wav
/usr/games/layouts/FunKey/sounds/load.wav /usr/games/layouts/Flat/sounds/load.wav
/usr/games/layouts/FunKey/sounds/load.wav /usr/games/layouts/PixxelPlus/sounds/load.wav
/usr/games/layouts/FunKey/sounds/load.wav /usr/games/layouts/Superlopez/sounds/load.wav
/usr/games/layouts/FunKey/sounds/load.wav /usr/games/layouts/TFT/sounds/load.wav
/usr/games/layouts/FunKey/sounds/unload.wav /usr/games/layouts/Classic/sounds/unload.wav
/usr/games/layouts/FunKey/sounds/unload.wav /usr/games/layouts/Flat/sounds/unload.wav
/usr/games/layouts/FunKey/sounds/unload.wav /usr/games/layouts/PixxelPlus/sounds/unload.wav
/usr/games/layouts/FunKey/sounds/unload.wav /usr/games/layouts/Superlopez/sounds/unload.wav
/usr/games/layouts/FunKey/sounds/unload.wav /usr/games/layouts/TFT/sounds/unload.wav
/usr/share/gmenu2x/skins/240x240/DrUm3x3/wallpapers/bg-mame.png /usr/share/gmenu2x/skins/240x240/DrUm3x4/wallpapers/bg-mame.png
EOF
