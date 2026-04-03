
# Market Data Receiver Project

This project demonstrates the use of SYCL, ZeroMQ, and QuickFIX to receive and process market data. The system connects to a ZeroMQ channel for market data transmission and uses QuickFIX for message parsing. The program also uses DPC++ for running FPGA/CPU/GPU tasks.

## Installing

`bash
pip install dxcore
`

## Prerequisites

Ensure the following are installed and set up:

### Development

Configuring build
```bash
git clone git@github.com:divergex/dxcore
cd dxcore
```

Configuring CMake
```bash
cmake -DCMAKE_BUILD_TYPE=Debug -DCMAKE_MAKE_PROGRAM=ninja -G Ninja -S ./ -B cmake-build-debug 
```

Building dxcore
```bash
cmake --build cmake-build-debug --target dxcore -j $(nproc)
```

Building python bindings
```bash
cmake --build cmake-build-debug --target python -j $(nproc)
```

Installing python package
```bash
pip install -e .
```

Local build test
```bash
act push -b --artifact-server-path=/tmp/artifacts
```