# Makefile for FunKey-OS
#
# Copyright (C) 2020 by Michel Stempin <michel.stempin@funkey-project.com>
#
# This program is free software; you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation; either version 2 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
# General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program; if not, write to the Free Software
# Foundation, Inc., 59 Temple Place, Suite 330, Boston, MA 02111-1307 USA
#

BRMAKE = buildroot/utils/brmake -C buildroot
BR = make -C buildroot

SOURCE_DATE_EPOCH ?= $(shell git log -1 --format=%ct 2>/dev/null || echo 0)
E2FSPROGS_FAKE_TIME ?= $(SOURCE_DATE_EPOCH)
FUNKEY_GIT_DIRTY = $(shell test -z "$$(git status --porcelain --untracked-files=normal 2>/dev/null)" || printf '%s' -dirty)
FUNKEY_GIT_REV ?= $(shell git rev-parse --short=12 HEAD 2>/dev/null || echo unknown)$(FUNKEY_GIT_DIRTY)
FUNKEY_VERSION ?= 2.3.0-spaceghost.g$(FUNKEY_GIT_REV)
export SOURCE_DATE_EPOCH E2FSPROGS_FAKE_TIME FUNKEY_VERSION

ZIG_HOST_BIN := $(abspath FunKey/output/zig-host/bin)
ZIG_HOST_CC := $(ZIG_HOST_BIN)/zig-host-cc
ZIG_HOST_CXX := $(ZIG_HOST_BIN)/zig-host-c++

# Strip quotes and then whitespaces
qstrip = $(strip $(subst ",,$(1)))
#"))

# MESSAGE Macro -- display a message in bold type
MESSAGE = echo "$(shell date +%Y-%m-%dT%H:%M:%S) $(TERM_BOLD)\#\#\# $(call qstrip,$(1))$(TERM_RESET)"
TERM_BOLD := $(shell tput smso 2>/dev/null)
TERM_RESET := $(shell tput rmso 2>/dev/null)

.PHONY: all firmware prepare-buildroot fun fun-recovery fun-funkey source image update checksums package-image package-update package-checksums package-inventory usb-variants defconfig clean distclean print-version zig-host zig-defconfig zig-cc zig-restore zig-all zig-variants

.IGNORE: _Makefile_

all: zig-all
	@:

firmware: checksums package-inventory
	@:

checksums: image update
	@$(MAKE) --no-print-directory package-checksums FUNKEY_VERSION='$(FUNKEY_VERSION)'

package-checksums:
	@$(call MESSAGE,"Creating artifact checksums")
	@cd images && \
	sha256sum \
		FunKey-rootfs-$(FUNKEY_VERSION).fwu \
		FunKey-sdcard-$(FUNKEY_VERSION).img.xz \
		> SHA256SUMS-$(FUNKEY_VERSION).txt.tmp && \
	mv SHA256SUMS-$(FUNKEY_VERSION).txt.tmp SHA256SUMS-$(FUNKEY_VERSION).txt
	@./scripts/firmware-size-report '$(FUNKEY_VERSION)'

package-inventory: FunKey/output/.config Recovery/output/.config
	@$(call MESSAGE,"Creating software package inventories")
	@mkdir -p images
	@$(BR) --no-print-directory BR2_EXTERNAL=../FunKey O=../FunKey/output show-info \
		> images/packages-FunKey-$(FUNKEY_VERSION).json.raw
	@sed -n '/^[[:space:]]*{/p' images/packages-FunKey-$(FUNKEY_VERSION).json.raw \
		> images/packages-FunKey-$(FUNKEY_VERSION).json.tmp
	@$(BR) --no-print-directory BR2_EXTERNAL=../Recovery O=../Recovery/output show-info \
		> images/packages-Recovery-$(FUNKEY_VERSION).json.raw
	@sed -n '/^[[:space:]]*{/p' images/packages-Recovery-$(FUNKEY_VERSION).json.raw \
		> images/packages-Recovery-$(FUNKEY_VERSION).json.tmp
	@rm -f images/packages-FunKey-$(FUNKEY_VERSION).json.raw \
		images/packages-Recovery-$(FUNKEY_VERSION).json.raw
	@mv images/packages-FunKey-$(FUNKEY_VERSION).json.tmp images/packages-FunKey-$(FUNKEY_VERSION).json
	@mv images/packages-Recovery-$(FUNKEY_VERSION).json.tmp images/packages-Recovery-$(FUNKEY_VERSION).json
	@./scripts/package-inventory-report \
		images/packages-FunKey-$(FUNKEY_VERSION).json \
		images/packages-FunKey-$(FUNKEY_VERSION).txt
	@./scripts/package-inventory-report \
		images/packages-Recovery-$(FUNKEY_VERSION).json \
		images/packages-Recovery-$(FUNKEY_VERSION).txt

print-version:
	@printf '%s\n' '$(FUNKEY_VERSION)'

zig-host:
	@./scripts/install-zig-host-tools $(ZIG_HOST_BIN)

zig-defconfig: zig-host
	+@PATH="$(ZIG_HOST_BIN):$$PATH" HOSTCC="$(ZIG_HOST_CC)" HOSTCXX="$(ZIG_HOST_CXX)" \
		$(MAKE) FunKey/funkey_defconfig Recovery/recovery_defconfig

zig-cc: zig-host prepare-buildroot
	+@PATH="$(ZIG_HOST_BIN):$$PATH" HOSTCC="$(ZIG_HOST_CC)" HOSTCXX="$(ZIG_HOST_CXX)" \
		$(MAKE) FunKey/toolchain Recovery/toolchain
	@./scripts/install-zig-cc FunKey/output
	@./scripts/install-zig-cc Recovery/output

zig-restore:
	@./scripts/install-zig-cc --restore FunKey/output
	@./scripts/install-zig-cc --restore Recovery/output

zig-all: zig-cc
	+@PATH="$(ZIG_HOST_BIN):$$PATH" HOSTCC="$(ZIG_HOST_CC)" HOSTCXX="$(ZIG_HOST_CXX)" \
		$(MAKE) firmware

zig-variants: zig-all
	@./scripts/build-usb-network-variant '$(FUNKEY_VERSION)'

usb-variants: checksums
	@./scripts/build-usb-network-variant '$(FUNKEY_VERSION)'

_Makefile_:
	@:

%/Makefile:
	@:

buildroot: buildroot/.git
	@:

buildroot/.git:
	@$(call MESSAGE,"Getting buildroot")
	@git submodule init
	@git submodule update

prepare-buildroot: buildroot/.git
	@./scripts/prepare-buildroot

fun: fun-recovery fun-funkey
	@$(call MESSAGE,"Making fun")

fun-recovery: prepare-buildroot Recovery/output/.config
	@$(call MESSAGE,"Making fun in Recovery")
	+@$(BRMAKE) BR2_EXTERNAL=../Recovery O=../Recovery/output

fun-funkey: prepare-buildroot FunKey/output/.config
	@$(call MESSAGE,"Making fun in FunKey")
	+@$(BRMAKE) BR2_EXTERNAL=../FunKey O=../FunKey/output

sdk: buildroot SDK/output/.config
	@$(call MESSAGE,"Making FunKey SDK")
	+@$(BRMAKE) BR2_EXTERNAL=../SDK O=../SDK/output prepare-sdk
	@$(call MESSAGE,"Generating SDK tarball")
	@export LC_ALL=C; \
	SDK=FunKey-sdk-DrUm78; \
	grep -lr "$(shell pwd)/SDK/output/host" SDK/output/host | while read -r FILE ; do \
		if file -b --mime-type "$${FILE}" | grep -q '^text/'; then \
			sed -i "s|$(shell pwd)/SDK/output/host|/opt/$${SDK}|g" "$${FILE}"; \
		fi; \
	done; \
	mkdir -p images; \
	tar czf "images/$${SDK}.tar.gz" \
		--owner=0 --group=0 --numeric-owner \
		--transform="s#^$(patsubst /%,%,$(shell pwd))/SDK/output/host#$${SDK}#" \
		-C / "$(patsubst /%,%,$(shell pwd))/SDK/output/host"; \
	rm -f download/toolchain-external-custom/$${SDK}.tar.gz; \
	mkdir -p download/toolchain-external-custom; \
	ln -s ../../images/$${SDK}.tar.gz download/toolchain-external-custom/

FunKey/%: FunKey/output/.config
	@$(call MESSAGE,"Making $(notdir $@) in $(subst /,,$(dir $@))")
	+@$(BR) BR2_EXTERNAL=../FunKey O=../FunKey/output $(notdir $@)

Recovery/%: Recovery/output/.config
	@$(call MESSAGE,"Making $(notdir $@) in $(subst /,,$(dir $@))")
	+@$(BR) BR2_EXTERNAL=../Recovery O=../Recovery/output $(notdir $@)

SDK/%: SDK/output/.config
	@$(call MESSAGE,"Making $(notdir $@) in $(subst /,,$(dir $@))")
	+@$(BR) BR2_EXTERNAL=../SDK O=../SDK/output $(notdir $@)

#%: FunKey/output/.config
#	@$(call MESSAGE,"Making $@ in FunKey")
#	@$(BR) BR2_EXTERNAL=../FunKey O=../FunKey/output $@

source:
	@$(call MESSAGE,"Getting sources")
	+@$(BR) BR2_EXTERNAL=../SDK O=../SDK/output source
	+@$(BR) BR2_EXTERNAL=../Recovery O=../Recovery/output source
	+@$(BR) BR2_EXTERNAL=../FunKey O=../FunKey/output source

image: fun
	@$(MAKE) --no-print-directory package-image FUNKEY_VERSION='$(FUNKEY_VERSION)'

# Packaging-only entry points deliberately do not depend on `fun`. They are
# used to derive profiles that only alter files inside completed rootfs images.
package-image:
	@$(call MESSAGE,"Creating disk image")
	@./scripts/package-sdcard-image '$(FUNKEY_VERSION)'

image-prod: fun
	@$(call MESSAGE,"Creating production disk image")
	@./scripts/package-sdcard-image '$(FUNKEY_VERSION)' \
		genimage-prod.cfg sdcard-prod.img \
		'FunKey-sdcard-prod-$(FUNKEY_VERSION)' none

update: fun
	@$(MAKE) --no-print-directory package-update FUNKEY_VERSION='$(FUNKEY_VERSION)'

package-update:
	@$(call MESSAGE,"Creating update file")
	@rm -rf tmp-update
	@mkdir -p tmp-update
	@sed 's/@FUNKEY_VERSION@/$(FUNKEY_VERSION)/g' \
		FunKey/board/funkey/sw-description > tmp-update/sw-description
	@cp FunKey/board/funkey/update_partition tmp-update/
	@cd FunKey/output/images && \
	rm -f rootfs.ext2.gz && \
	gzip -n -k rootfs.ext2 &&\
	mv rootfs.ext2.gz ../../../tmp-update/
	@touch -h -d "@$(SOURCE_DATE_EPOCH)" tmp-update/*
	@cd tmp-update && \
	echo sw-description rootfs.ext2.gz update_partition | \
	tr " " "\n" | \
	cpio --reproducible --owner=0:0 -o -H crc --quiet > ../images/FunKey-rootfs-$(FUNKEY_VERSION).fwu
	@rm -rf tmp-update

defconfig:
	@$(call MESSAGE,"Updating default configs")
	@$(call MESSAGE,"Updating default configs in SDK")
	+@$(BR) BR2_EXTERNAL=../SDK O=../SDK/output savedefconfig
	@$(call MESSAGE,"Updating default configs in Recovery")
	+@$(BR) BR2_EXTERNAL=../Recovery O=../Recovery/output savedefconfig linux-update-defconfig uboot-update-defconfig busybox-update-config
	@$(call MESSAGE,"Updating default configs in FunKey")
	+@$(BR) BR2_EXTERNAL=../FunKey O=../FunKey/output savedefconfig linux-update-defconfig busybox-update-config

clean:
	@$(call MESSAGE,"Clean everything")
	+@$(BR) BR2_EXTERNAL=../SDK O=../SDK/output distclean
	+@$(BR) BR2_EXTERNAL=../Recovery O=../Recovery/output distclean
	+@$(BR) BR2_EXTERNAL=../FunKey O=../FunKey/output distclean
	@rm -f br.log

distclean: clean
	@$(call MESSAGE,"Really clean everything")
	@rm -rf download images

FunKey/output/.config:
	@$(call MESSAGE,"Configure FunKey")
	@mkdir -p FunKey/board/funkey/patches
	+@$(BR) BR2_EXTERNAL=../FunKey O=../FunKey/output funkey_defconfig

Recovery/output/.config:
	@$(call MESSAGE,"Configure Recovery")
	@mkdir -p Recovery/board/funkey/patches
	+@$(BR) BR2_EXTERNAL=../Recovery O=../Recovery/output recovery_defconfig

SDK/output/.config:
	@$(call MESSAGE,"Configure SDK")
	@mkdir -p SDK/board/funkey/patches
	+@$(BR) BR2_EXTERNAL=../SDK O=../SDK/output funkey_defconfig
