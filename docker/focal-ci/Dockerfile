# syntax=docker/dockerfile:1

# Keep the GCC 9-era userspace expected by this Buildroot tree, but pin the
# actual base manifest so firmware CI cannot silently move underneath us.
FROM ubuntu:20.04@sha256:8feb4d8ca5354def3d8fce243717141ce31e2c428701f6682bd2fafe15388214

ARG DEBIAN_FRONTEND=noninteractive
ARG ZIG_VERSION=0.16.0
ARG ZIG_SHA256=70e49664a74374b48b51e6f3fdfbf437f6395d42509050588bd49abe52ba3d00
ARG IMAGE_REVISION=unversioned

LABEL org.opencontainers.image.title="FunKey-OS Focal build environment" \
      org.opencontainers.image.description="Pinned Ubuntu 20.04 and Zig environment for reproducible FunKey-OS Buildroot CI" \
      org.opencontainers.image.source="https://github.com/Spaceghost/FunKey-OS" \
      org.opencontainers.image.licenses="GPL-2.0-or-later" \
      org.opencontainers.image.version="focal-zig-${ZIG_VERSION}-v1" \
      org.opencontainers.image.revision="${IMAGE_REVISION}"

RUN apt-get update \
 && apt-get install -y -q --no-install-recommends \
      bash \
      bc \
      binutils \
      build-essential \
      busybox \
      bzip2 \
      ca-certificates \
      cmake \
      cpio \
      cvs \
      expect \
      file \
      gawk \
      git \
      gzip \
      liblscp-dev \
      libncurses5-dev \
      locales \
      mercurial \
      openssh-client \
      patch \
      perl \
      procps \
      python3 \
      python3-dev \
      python3-distutils \
      python3-setuptools \
      rsync \
      subversion \
      tar \
      unzip \
      wget \
      xxd \
      xz-utils \
      zstd \
 && ln -sfn /usr/bin/python3 /usr/local/bin/python \
 && locale-gen en_US.UTF-8 \
 && rm -rf /var/lib/apt/lists/*

RUN wget -q \
      "https://ziglang.org/download/${ZIG_VERSION}/zig-x86_64-linux-${ZIG_VERSION}.tar.xz" \
      -O /tmp/zig.tar.xz \
 && printf '%s  %s\n' "${ZIG_SHA256}" /tmp/zig.tar.xz | sha256sum -c - \
 && mkdir -p /opt/zig \
 && tar -xJf /tmp/zig.tar.xz --strip-components=1 -C /opt/zig \
 && ln -s /opt/zig/zig /usr/local/bin/zig \
 && rm -f /tmp/zig.tar.xz

ENV LANG=en_US.UTF-8 \
    LC_ALL=en_US.UTF-8 \
    FORCE_UNSAFE_CONFIGURE=1 \
    FUNKEY_CI_IMAGE_REVISION=${IMAGE_REVISION}

RUN test "$(zig version)" = "${ZIG_VERSION}" \
 && python --version \
 && locale -a | grep -qx 'en_US.utf8'

WORKDIR /workspace
