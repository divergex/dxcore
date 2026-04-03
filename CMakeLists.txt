cmake_minimum_required(VERSION 3.25)

message(STATUS "Conan: Installing dependencies...")

if(NOT CMAKE_BUILD_TYPE)
    set(CONAN_BUILD_TYPE "Debug")
else()
    set(CONAN_BUILD_TYPE "${CMAKE_BUILD_TYPE}")
endif()

execute_process(
        COMMAND conan install .
        --build=missing
        -of "${CMAKE_BINARY_DIR}"
        -s build_type=${CONAN_BUILD_TYPE}
        WORKING_DIRECTORY "${CMAKE_SOURCE_DIR}"
        RESULT_VARIABLE CONAN_RESULT
)
if(NOT CONAN_RESULT EQUAL 0)
     message(FATAL_ERROR "Conan install failed with code ${CONAN_RESULT}")
endif()

message(STATUS "Conan: Dependencies installed to ${CMAKE_BINARY_DIR}")

if(EXISTS "${CMAKE_BINARY_DIR}/build/${CONAN_BUILD_TYPE}/generators/conan_toolchain.cmake")
    set(CONAN_GENERATORS_DIR "${CMAKE_BINARY_DIR}/build/${CONAN_BUILD_TYPE}/generators")
elseif(EXISTS "${CMAKE_BINARY_DIR}/build/generators/conan_toolchain.cmake")
    set(CONAN_GENERATORS_DIR "${CMAKE_BINARY_DIR}/build/generators")
elseif(EXISTS "${CMAKE_BINARY_DIR}/conan_toolchain.cmake")
    set(CONAN_GENERATORS_DIR "${CMAKE_BINARY_DIR}")
else()
    message(FATAL_ERROR "Could not find conan_toolchain.cmake in any expected location")
endif()

message(STATUS "Conan generators directory: ${CONAN_GENERATORS_DIR}")

include("${CONAN_GENERATORS_DIR}/conan_toolchain.cmake")

# --------------------------------------------------------------------- #

project(dxcore LANGUAGES CXX)

list(APPEND CMAKE_PREFIX_PATH "${CONAN_GENERATORS_DIR}")
list(APPEND CMAKE_MODULE_PATH "${CONAN_GENERATORS_DIR}")

set(gtest_force_shared_crt ON CACHE BOOL "" FORCE)
set(INSTALL_GTEST OFF CACHE BOOL "" FORCE)
set(BUILD_GMOCK ON CACHE BOOL "" FORCE)
set(BUILD_GTEST ON CACHE BOOL "" FORCE)

add_subdirectory(src)
add_subdirectory(bindings/python)
add_subdirectory(tests)