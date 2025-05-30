cmake_minimum_required(VERSION 3.15)

# Force use of Intel DPC++ compiler if available
find_program(ICPX_EXECUTABLE icpx)
if(ICPX_EXECUTABLE)
    set(CMAKE_CXX_COMPILER ${ICPX_EXECUTABLE} CACHE STRING "DPC++ compiler" FORCE)
else()
    message(FATAL_ERROR "Intel SYCL compiler 'icpx' not found. Please install Intel oneAPI or another SYCL compiler.")
endif()

project(dxcore LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(ZeroMQ REQUIRED CONFIG)
find_package(cppzmq REQUIRED)
find_package(MPI REQUIRED)
find_package(Catch2 3 REQUIRED)
set(PYBIND11_FINDPYTHON ON)
find_package(pybind11 REQUIRED)

pybind11_add_module(dxcore src/bindings.cpp)

add_subdirectory(src/core)

target_link_libraries(dxcore
    PRIVATE
    libzmq-static
    cppzmq
    openmpi::openmpi
    core
)

add_executable(tests test/test_lob.cpp)
target_link_libraries(tests 
    PRIVATE 
    Catch2::Catch2WithMain 
    core
)


target_include_directories(dxcore PRIVATE ${MPI_CXX_INCLUDE_DIRS})
if(MPI_COMPILE_FLAGS)
    target_compile_options(dxcore PRIVATE ${MPI_COMPILE_FLAGS})
endif()
if(MPI_LINK_FLAGS)
    target_link_options(dxcore PRIVATE ${MPI_LINK_FLAGS})
endif()

target_compile_options(dxcore PRIVATE -fsycl)
target_link_options(dxcore PRIVATE -fsycl)

if(CMAKE_CXX_COMPILER_ID MATCHES "Clang|GNU|IntelLLVM")
    foreach(target core dxcore)
        target_compile_options(${target} PRIVATE
            -Wall
            -Wextra
            -Wpedantic
            -Wconversion
            -Wsign-conversion
            -Wunused-parameter
            -fcoroutines
        )
    endforeach()
endif()

