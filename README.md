
# Market Data Receiver Project

This project demonstrates the use of SYCL, ZeroMQ, and QuickFIX to receive and process market data. The system connects to a ZeroMQ channel for market data transmission and uses QuickFIX for message parsing. The program also uses DPC++ for running FPGA/CPU/GPU tasks.

## Prerequisites

Ensure the following are installed and set up:

### 1. **vcpkg** (C++ package manager)
- Install **vcpkg**:
  Follow the [vcpkg installation guide](https://github.com/microsoft/vcpkg/blob/master/docs/index.md) to set up vcpkg on your system.

### 2. **Dependencies**:
- **SYCL** (DPC++ or other SYCL implementations)
- **ZeroMQ**
- **QuickFIX**

To install the required libraries via **vcpkg**, use the following commands:

```bash
# Install SYCL (Intel DPC++ example, adjust for your specific SYCL version)
vcpkg install intel-dpcpp

# Install ZeroMQ
vcpkg install zeromq

# Install QuickFIX
vcpkg install quickfix
