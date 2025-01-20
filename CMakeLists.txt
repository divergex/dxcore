cmake_minimum_required(VERSION 3.10)
project(dxcore)

set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED True)

project(MarketData)

# Set C++ standard
set(CMAKE_CXX_STANDARD 23)

# Find required packages using vcpkg
find_package(SYCL REQUIRED)
find_package(ZeroMQ REQUIRED)
find_package(QuickFIX REQUIRED)

# Add executable
add_executable(MarketData src/feed/exchange.cpp)

# Link the libraries
target_link_libraries(MarketData PRIVATE
        SYCL::SYCL
        zmq
        QuickFIX::QuickFIX
)

# Optional: Set include directories (if needed)
# target_include_directories(MarketDataReceiver PRIVATE ${CONAN_INCLUDE_DIRS})
