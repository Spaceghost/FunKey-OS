################################################################################
#
# gmu
#
################################################################################

GMU_VERSION = HEAD
GMU_SITE_METHOD = git
GMU_SITE = https://github.com/DrUm78/gmu.git
GMU_LICENSE = GPL-2.0
GMU_LICENSE_FILES = LICENSE

GMU_DEPENDENCIES = sdl

define GMU_PREPARE_TOOLCHAIN_ALIASES
	rm -rf $(@D)/toolchain
	mkdir -p $(@D)/toolchain/bin
	for tool in $(HOST_DIR)/bin/arm-linux-*; do \
		[ -e "$$tool" ] || continue; \
		name=$${tool##*/arm-linux-}; \
		ln -s "$$tool" "$(@D)/toolchain/bin/arm-funkey-linux-musleabihf-$$name"; \
	done
endef

define GMU_BUILD_CMDS
	$(GMU_PREPARE_TOOLCHAIN_ALIASES)
	(cd $(@D); \
	sed -i -e 's|rm -rf|#rm -rf|g' package; \
	sed -i -e 's|make -f Makefile.funkey clean|#make -f Makefile.funkey clean|g' package; \
	chmod +x package; \
	PATH=$(@D)/toolchain/bin:$(HOST_DIR)/bin:$(PATH) \
	PKG_CONFIG_SYSROOT_DIR=$(STAGING_DIR) \
	PKG_CONFIG_LIBDIR=$(STAGING_DIR)/usr/lib/pkgconfig:$(STAGING_DIR)/usr/share/pkgconfig \
	./package \
	)
endef

define GMU_INSTALL_TARGET_CMDS
	$(INSTALL) -d -m 0755 $(TARGET_DIR)/usr/bin/gmu
	$(INSTALL) -D -m 0755 -t $(TARGET_DIR)/usr/bin/gmu $(@D)/gmu
	$(INSTALL) -D -m 0755 -t $(TARGET_DIR)/usr/bin/gmu $(@D)/opk/*
	$(INSTALL) -D -m 0755 -t $(TARGET_DIR)/usr/bin/gmu/decoders $(@D)/decoders/*
	$(INSTALL) -D -m 0755 -t $(TARGET_DIR)/usr/bin/gmu/frontends $(@D)/frontends/*
	$(INSTALL) -D -m 0755 -t $(TARGET_DIR)/usr/bin/gmu/themes/dbcompo-small $(@D)/themes/dbcompo-small/*
	$(INSTALL) -D -m 0755 -t $(TARGET_DIR)/usr/bin/gmu/themes/dbcompo-large $(@D)/themes/dbcompo-large/*
	$(INSTALL) -D -m 0755 -t $(TARGET_DIR)/usr/bin/gmu/themes/default-modern-small $(@D)/themes/default-modern-small/*
	$(INSTALL) -D -m 0755 -t $(TARGET_DIR)/usr/bin/gmu/themes/default-modern-large $(@D)/themes/default-modern-large/*
	rm -f $(TARGET_DIR)/usr/bin/gmu/gmu.funkey-s.desktop
endef

define GMU_CREATE_OPK
	$(INSTALL) -d -m 0755 $(TARGET_DIR)/usr/local/share/OPKs/Applications
	$(HOST_DIR)/usr/bin/mksquashfs $(GMU_PKGDIR)/opk $(TARGET_DIR)/usr/local/share/OPKs/Applications/gmu_funkey-s.opk -all-root -noappend -no-exports -no-xattrs
endef
GMU_POST_INSTALL_TARGET_HOOKS += GMU_CREATE_OPK

$(eval $(generic-package))
