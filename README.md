# FunKey OS for RG Nano

## Intro
This is Spaceghost's RG Nano fork of
[DrUm78/FunKey-OS](https://github.com/DrUm78/FunKey-OS). It contains the
bootloader, Linux system, launcher, and emulator sources required to build
FunKey OS for the Anbernic RG Nano while preserving the original project's
history and licensing.

The RG Nano uses an Allwinner V3s ARM Cortex-A7 processor. FunKey OS provides
the Linux-based hardware support and compact gaming environment used by this
port.

FunKey OS is based on Linux, and is built from scratch using the [buildroot](http://nightly.buildroot.org/) tool that simplifies and automates the process of building a complete Linux system for an embedded system like this.

Technically, Funkey OS is a [buildroot (v2) based external tree](https://buildroot.org/downloads/manual/manual.html#outside-br-custom) for building the bootloader, the Linux kernel and user utilities, as well as the optimized retro-game launcher and console emulators.

## Build host requirements
The resulting SD Card image is about 451 MiB and the compressed firmware update
is about 127 MiB. Sources and intermediate build products are much larger; keep
at least 20 GiB free for a complete build.

And even if the resulting FunKey OS boots in less than 5s, it still requires a considerable amount of time to compile: please account for 1 1/2 hour on a modern multi-core CPU with SSD drives and a decent Internet bandwidth.

As the target CPU is probably different from the one running on your build host machine, a process known as [_cross-compilation_](https://en.wikipedia.org/wiki/Cross_compiler) is required for the build, and as the target system will eventually be Linux, this is much better handled on hosts running a Linux-based operating system too.

As a matter of fact, the FunKey OS is meant to be built on a native Ubuntu or Debian Linux host machine (Ubuntu 20.04 LTS in our case, but this should also work with other versions, too). And with only a few changes to the prerequisites, it can certainly be adapted to build on other common Linux distros.

However, if your development machine does not match this setup, there are still several available solutions:
 -  use a lightweight container system such as [Docker](https://www.docker.com/) and run an Ubuntu or Debian Linux container in it
 - use a VM (Virtual Machine) , such as provided by [VirtualBox](https://www.virtualbox.org/) and run an Ubuntu or Debian Linux in it
 - for Windows 10/11 users, use the [WSL2](https://learn.microsoft.com/en-us/windows/wsl/install) (Windows System for Linux 2) subsystem and run an Ubuntu Linux distro in it

In order to install one of these virtualized environments on your machine, please refer to the corresponding documentation.

## Build on a Physical/Virtual Machine

### Prerequisites
While Buildroot itself will build most host packages it needs for the compilation, some standard Linux utilities are expected to be already installed on the host system. If not already present, you will need to install the following packages beforehand:
 - bash
 - bc
 - binutils
 - build-essential
 - bzip2
 - ca-certificates
 - cpio
 - cvs
 - expect
 - file
 - g++
 - gcc
 - git
 - gzip
 - liblscp-dev
 - libncurses5-dev
 - locales
 - make
 - mercurial
 - openssh-client
 - patch
 - perl
 - procps
 - python
 - python-dev
 - python3
 - python3-dev
 - python3-distutils
 - python3-setuptools
 - rsync
 - rsync
 - sed
 - subversion
 - sudo
 - tar
 - unzip
 - wget
 - which
 - xxd

On Ubuntu/Debian Linux, this is achieved by running the following command:
```bash
$ sudo apt install bash bc binutils build-essential bzip2 ca-certificates cpio cvs expect file g++ gcc git gzip liblscp-dev libncurses5-dev locales make mercurial openssh-client patch perl procps python python-dev python3 python3-dev python3-distutils python3-setuptools rsync rsync sed subversion sudo tar unzip wget which xxd
```

### How to get the sources
When using either physical or virtual Linux machines, you must clone the FunKey OS repository from Github (here we place it into a `FunKey-OS` directory):

```bash
$ git clone https://github.com/Spaceghost/FunKey-OS.git FunKey-OS
$ cd FunKey-OS
$ git switch docs/rg-nano-fork-identity
```

`docs/rg-nano-fork-identity` contains the current software-validated
development stack. Development and optimization work is published in named
branches before it is integrated into `rg-nano-next`; physical RG Nano
validation is still required before treating development artifacts as a
release.

### Build the disk image & firmware update files
Build the RG Nano SD image, update file, and checksums with:

```bash
$ make -j"$(nproc)" all
```
Run `make sdk` separately only if you also need the cross-development SDK.
This may take a while (~1h30), so consider getting yourself a cup, a glass or a bottle of your favorite beverage ;-)

<ins>Note</ins>: you will need to have access to the network, since buildroot will download the package sources.

### Result of the build
After building, you should obtain `FunKey-sdcard-<version>.img` and
`FunKey-rootfs-<version>.fwu` in the `images` directory, together with a
matching `SHA256SUMS-<version>.txt`. Run `make print-version` to show the
version embedded in both artifacts. Verify copied or downloaded files with:

```bash
$ version=$(make -s print-version)
$ cd images && sha256sum -c "SHA256SUMS-${version}.txt"
```

## Build in a container

### Prerequisites
When using a Docker container, all the prerequisites are automatically installed.

### How to get the sources
Clone the fork and select the RG Nano branch as described above. Build the
included Dockerfile from the repository root; the branch passed to Docker is
the branch it will clone and compile:
```bash
$ docker build -f docker/Dockerfile \
    --build-arg FUNKEY_OS_REF="$(git branch --show-current)" \
    -t spaceghost/funkey-os .
```

### Build the disk image & firmware update files
Build the firmware artifacts with:
```bash
$ docker run --name funkey-os spaceghost/funkey-os
```

Or alternatively, you can run it in the background with:
```bash
$ docker run -d --name funkey-os spaceghost/funkey-os
```

If you launch it in the background, you can still follow what is going on with either:
```bash
$ docker top funkey-os
```
Or:
```bash
$ docker logs funkey-os
```

This may take a while (~1h30), so consider getting yourself a cup, a glass or a bottle of your favorite beverage ;-)

<ins>Note</ins>: you will need to have access to the network, since buildroot will download the package sources.

### Result of the build
After building, you can copy the versioned SD Card image, firmware update, and
checksum manifest from the container into the host current directory:
```bash
$ mkdir images
$ docker cp funkey-os:/home/funkey/FunKey-OS/images/. images/
```

## How to write to the SD card
You can copy the versioned bootable SD Card image onto an SD card using "dd":

```bash
$ version=$(make -s print-version)
$ sudo dd if="images/FunKey-sdcard-${version}.img" of=/dev/sdX
```
<ins>Warning</ins>: Please make sure that */dev/sdX* device corresponds to your SD Card, otherwise you may wipe out one of your hard drive partitions!

Alternatively, you can use the Balena-Etcher graphical tool to burn the image
to the SD card safely and on any platform:

https://www.balena.io/etcher/

Once the SD card is written, insert it into your RG Nano, and
power it up. Your new system should come up now and start a console on
the UART0 serial port and display the retro game launcher on the graphical screen.

## How to update the RG Nano firmware
It is possible to update an RG Nano over its USB-C data port:
 - Connect the RG Nano console to your host machine using a USB data cable
 - From the retro-game launcher, press the **ON/OFF** button to access the menu
 - Using the **Up/Down** keys, select the "**MOUNT USB**" screen and press the "**A**" key twice to mount the RG Nano on your machine as a USB mass-storage drive
 - Copy exactly one `images/FunKey-rootfs-<version>.fwu` file to the top level of the shared drive
 - When finished, eject the USB mass storage from your host machine
 - Back on the RG Nano console, press the "**A**" key twice to unmount the USB mass-storage drive
 - The RG Nano will automatically detect the firmware update file and install it before returning to the retro-game launcher

If more than one `FunKey-*.fwu` file is present, the RG Nano refuses to choose
between them. Reconnect the shared drive and keep only the update you intend to
install. Failed update files are preserved so they can be inspected or replaced.
