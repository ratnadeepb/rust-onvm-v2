# Attempt to migrate openNetVM to Rust

### Detour to DPDK FFI
I was working on this repo to build a generic DPDK FFI interface for Rust. Some way down the line, I realised that I was re-inventing a lot of the wheel. Thus, I have had to take a little detour to understand async Rust (async-std, Futures, Tokio and mio) better. This would lead to a more stable runtime. The capsule team is working on the same thing right now. So I might again come back and fork off their work if possible, otherwise I am going to implement as much of a runtime as I possibly can.

Some of that work can be found here: https://github.com/ratnadeepb/dpdk-ffi

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
echo "if [ -d $HOME/dpdk-stable-19.11.4 ]; then
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
meson build
cd build && ninja && sudo ninja install
```

#### Export the Libraries

```bash
export LD_LIBRARY_PATH=/usr/local/lib/x86_64-linux-gnu/
```

### Setup driver and Hugepages

```bash
sudo modprobe uio
cd $RTE_SDK
# Location of the driver depends on installation method
driver=$(find . -name igb_uio.ko | awk '{if (NR == 1) print$1}')
sudo insmod $driver
sudo modprobe igb_uio
curl -o dpdk_helper_scripts.sh https://raw.githubusercontent.com/sdnfv/openNetVM/master/scripts/dpdk_helper_scripts.sh
. dpdk_helper_scripts.sh
remove_igb_uio_module
set_numa_pages
```

## Makefile

The Makefile is rather simple as of now.</br>
By default, running `make` runs `cargo check` but so does `make check`. Similarly, there are options to `make debug`, `make release` and `make test` and they behave in manners suggested by the target names.

<b>As of now, I doubt running `make run` will be much helpful</b>

## Debugging

In the custom capsule-ffi, the bindgen layout tests are disabled. So only the tests in the onvm module run. So far in the absence of an executable, debugging is print statement based.

The tests run automatically with the `nocapture`, `show-output` and `quiet` options.

As a hack, the `onvm_run_init` test prints the environment variables it was passed out. The first env var is the name of executable that is running the test. So that executable can be passed to GDB/LLDB for further debugging. For example, the following can be done:

```bash
gdb --args ./target/debug/deps/onvm_mgr-0723e2bfe1aaa732 -- -l 0-3
```

The repo now contains a `onvm_mgr_test` binary that uses the `onvm_mgr` lib underneath. It can be debugged with:

```bash
sudo gdb --args ./target/debug/onvm_mgr_test -l 0-3 -n 2 --proc-type=primary --base-virtaddr=0x7f000000000
```

`gdb` can be replaced by `lldb` or `rust-lldb` or `rust-gdb` (these are the same as gdb and lldb).

## Lofty, long term Goals and Differences with openNetVM

Unlike openNetVM, which aims support run any generic network function, the goals for this project are to:
- use the openNetVM manager work as a Layer 2 switch.
- use the DPDK rings as a virtual interface to an associated sidecar proxy container
- run a TCP stack in the proxy container
- build yaml support in the proxy container to configure the proxy to forward packets to a backend container
- build support to implement network policies at both Layer 2 (openNetVM manager) and Layer 3 (the sidecar TCP stack)
