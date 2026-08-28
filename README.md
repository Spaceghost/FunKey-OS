<!-- ![FunKey OS Build](https://github.com/FunKey-Project/FunKey-OS/workflows/FunKey-OS%20Build/badge.svg) -->
# FunKey OS

## Intro
This repository contains all the sources required to build FunKey OS, the Open-Source firmware at the heart of the [FunKey S retro-gaming console](https://www.funkey-project.com/).

As the FunKey-S console is based on a sophisticated [Allwinner V3s ARM Cortex-A7 1.2GHz CPU](http://www.allwinnertech.com/index.php?c=product&a=index&id=38), an Operating System is mandatory in order to access all the hardware resources without re-inventing the wheel.

FunKey OS is based on Linux, and is built from scratch using the [buildroot](http://nightly.buildroot.org/) tool that simplifies and automates the process of building a complete Linux system for an embedded system like this.

Technically, Funkey OS is a [buildroot (v2) based external tree](https://buildroot.org/downloads/manual/manual.html#outside-br-custom) for building the bootloader, the Linux kernel and user utilities, as well as the optimized retro-game launcher and console emulators.

## Build host requirements
Even if the resulting disk image and firmware update files are relatively small (202 MB and 55MB, respectively), the size of the corresponding sources and the compilation by-products tend to be rather large, such that an available disk space of at least 12GB is required during the build.

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
$ git clone https://github.com/DrUm78/FunKey-OS.git FunKey-OS
```

Then enter into the created directory:

```bash
$ cd FunKey-OS
```

### Build the disk image & firmware update files
You may now build your FunKey with:

```bash
$ make sdk all
```
This may take a while (~1h30), so consider getting yourself a cup, a glass or a bottle of your favorite beverage ;-)

<ins>Note</ins>: you will need to have access to the network, since buildroot will download the package sources.

### Result of the build
After building, you should obtain `FunKey-sdcard-<version>.img` and
`FunKey-rootfs-<version>.fwu` in the `images` directory. Run
`make print-version` to show the version embedded in both artifacts.

## Build in a container

### Prerequisites
When using a Docker container, all the prerequisites are automatically installed.

### How to get the sources
When using a Docker container, you must first create a new directory (here we create a `FunKey-OS` directory) and get the FunKey OS [Dockerfile](https://github.com/DrUm78/FunKey-OS/blob/master/docker/Dockerfile):
```bash
$ mkdir FunKey-OS
$ cd FunKey-OS
$ wget https://raw.githubusercontent.com/DrUm78/FunKey-OS/master/docker/Dockerfile -o Dockerfile
```

You must then build the docker image (don't forget the final dot!):
```bash
$ docker build -t DrUm78/funkey-os .
```

### Build the disk image & firmware update files
You may now build your FunKey with:
```bash
$ docker run --name funkey-os DrUm78/funkey-os
```

Or alternatively, you can run it in the background with:
```bash
$ docker run -d --name funkey-os DrUm78/funkey-os
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
After building, you can copy the versioned SD Card image and firmware update
file from the container into the host current directory:
```bash
$ mkdir images
$ docker cp funkey-os:/home/funkey/FunKey-OS/images/FunKey-sdcard-2.3.0-spaceghost.gCOMMIT.img images/
$ docker cp funkey-os:/home/funkey/FunKey-OS/images/FunKey-rootfs-2.3.0-spaceghost.gCOMMIT.fwu images/
```

## How to write to the SD card
You can copy the bootable `images/sdcard.img` onto an SD card using "dd":

```bash
$ sudo dd if=images/FunKey-sdcard-2.3.0-spaceghost.gCOMMIT.img of=/dev/sdX
```
<ins>Warning</ins>: Please make sure that */dev/sdX* device corresponds to your SD Card, otherwise you may wipe out one of your hard drive partitions!

Alternatively, you can use the Balena-Etcher graphical tool to burn the image
to the SD card safely and on any platform:

https://www.balena.io/etcher/

Once the SD card is burnt, insert it into your FunKey S console slot, and
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
