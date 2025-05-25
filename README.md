
# Market Data Receiver Project

This project demonstrates the use of SYCL, ZeroMQ, and QuickFIX to receive and process market data. The system connects to a ZeroMQ channel for market data transmission and uses QuickFIX for message parsing. The program also uses DPC++ for running FPGA/CPU/GPU tasks.

## Prerequisites

Ensure the following are installed and set up:

### Development

Configuring build
```bash
git clone git@github.com:divergex/dxcore
cd dxcore
mkdir build && cd build
```

Installing conan dependencies:
```bash
conan install .. --build=missing -of .;
```

Configuring toolchain
```bash
cmake .. -DCMAKE_TOOLCHAIN_FILE=conan_toolchain.cmake -DCMAKE_BUILD_TYPE=Release -DCMAKE_PREFIX_PATH=$(pwd)
```

Building
```bash
cmake --build . -- -j$(nproc)
```
