# Attempt to migrate openNetVM to Rust

## openNetVM
This is an attempt to migrate openNetVM developed and maintained primarily by Dr. Timothy Wood and his team at the George Washington University.

openNetVM is a framework for building and managing virtual network functions. The framework wraps over DPDK, in what is traditionally known as the south side, and provides a simpler API to build and run network functions as processes or dockers in the north.

The primary difference between most other frameworks/libraries and openNetVM is that openNetVM provides the means to run a set of different network functions using a common set of APIs and a centralised manager that takes care of everything else but the actual responsibility of the network function.

The code and a fantastic set of documentation about the project can be found [here](https://github.com/sdnfv/openNetVM). Furthermore a wiki can be found [here](http://sdnfv.github.io/onvm/). There are some details to be found in the [NFV Wiki page](https://en.wikipedia.org/wiki/Network_function_virtualization) as well.

## Capsule - Rust NFV Library

Capsule is an NFV library written in Rust. As far as I have explored, the library appears to be expertly written, well maintained and works with the latest versions of DPDK. As part of this project, currently I am only using a slightly modified version of its DPDK FFI import.

Capsule can be further explored [here](https://lib.rs/crates/capsule).

## Installation

### Prerequisites

```bash

sudo apt update
sudo apt install build-essential linux-headers-$(uname -r) git libnuma-dev linux-modules-extra-$(uname -r) libclang-dev clang llvm-dev libpcap-dev


python3 --version
ret=`echo $3`
if [[ $ret -ne 0 ]]
  then sudo apt install python3
fi
```

### Choosing a network interface card (NIC) to use

```bash
lspci | awk '/net/ {print $1}' | xargs -i% lspci -ks %
```

### Install Ninja and Meson - DPDK build tools

```bash
sudo apt install ninja-build
```

Get meson from [here](https://github.com/mesonbuild/meson/releases/) - 0.55.3 is the latest meson build.</br>
<b>As of now, default Ubuntu 18.04 installation does not install the required meson version</b>

```bash
wget https://github.com/mesonbuild/meson/releases/download/0.55.3/meson-0.55.3.tar.gz
tar xvfh meson-0.55.3.tar.gz
cd meson-0.55.3 && sudo python3 setup.py install
```

### Hugepages

```bash
sudo su
echo "vm.nr_hugepages = 2048" >> /etc/sysctl.conf
sysctl -e -p
exit
```

### Install DPDK

Latest DPDK version can be found [here](http://core.dpdk.org/download/).

#### Basic Setup

```bash
wget https://fast.dpdk.org/rel/dpdk-19.11.4.tar.xz
tar xvfh dpdk-19.11.4.tar.xz
echo "if [ -d /mydata/dpdk-stable-19.11.4 ]; then
        export RTE_SDK=/mydata/dpdk-stable-19.11.4
fi" >> ~/.bashrc
echo "export RTE_TARGET=x86_64-native-linuxapp-gcc"  >> ~/.bashrc
source ~/.bashrc
cd $RTE_SDK
```

#### Using the older make system

```bash
EXTRA_CFLAGS=" -fPIC " make config T=$RTE_TARGET
EXTRA_CFLAGS=" -fPIC " make T=$RTE_TARGET -j 8
EXTRA_CFLAGS=" -fPIC " make install T=$RTE_TARGET -j 8
```

#### Using meson and ninja

```bash
EXTRA_CFLAGS=" -fPIC " meson build
cd build && EXTRA_CFLAGS=" -fPIC " ninja && sudo EXTRA_CFLAGS=" -fPIC " ninja install
```

### Setup driver and Hugepages

```bash
sudo modprobe uio
cd $RTE_SDK
# Location of the driver depends on installation method
driver=$(find . -name igb_uio.ko | awk '{if (NR == 1) print$1}')
sudo insmod $driver
sudo modprobe igb_uio
```

## Makefile

The Makefile is rather simple as of now.</br>
By default, running `make` runs `cargo check` but so does `make check`. Similarly, there are options to `make debug`, `make release` and `make run` and they behave in manners suggested by the target names.

<b>As of now, I doubt running `make run` will be much helpful</b>