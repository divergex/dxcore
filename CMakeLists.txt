cmake_minimum_required(VERSION 3.25)
project(dxcore CXX)

# ── dependencies ──────────────────────────────────────────────────────────────
include(FetchContent)

set(INSTALL_GTEST OFF CACHE BOOL "Disable installing googletest" FORCE)
FetchContent_Declare(
        googletest
        GIT_REPOSITORY https://github.com/google/googletest.git
        GIT_TAG v1.17.0
)
FetchContent_MakeAvailable(googletest)

# ── Conan automatically handled by conan_provider.cmake ───────────────────────
# include(${CMAKE_BINARY_DIR}/conan_provider.cmake OPTIONAL)

# ── core library ──────────────────────────────────────────────────────────────
add_library(dxcore STATIC
        src/dxcore.h
        src/core/order.cpp
        src/core/price_level.cpp
        src/core/order_book.cpp
)

target_include_directories(dxcore PUBLIC $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/src>)

# Link Conan’s Boost if available, fallback to system Boost otherwise
find_package(Boost REQUIRED)
target_link_libraries(dxcore PUBLIC Boost::headers)

target_compile_features(dxcore PUBLIC cxx_std_17)
set_target_properties(dxcore PROPERTIES POSITION_INDEPENDENT_CODE ON)

# ── tests ─────────────────────────────────────────────────────────────────────

enable_testing()

add_executable(dxcore-tests
        test/test_order.cpp
        test/test_price_level.cpp
        test/test_order_book.cpp
)

target_include_directories(dxcore-tests PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/src
)

target_link_libraries(dxcore-tests PRIVATE
        dxcore
        GTest::gtest_main
)

include(GoogleTest)
gtest_discover_tests(dxcore-tests)

# ── bindings ──────────────────────────────────────────────────────────────────
find_package(Python COMPONENTS Interpreter Development REQUIRED)
find_package(pybind11 REQUIRED)

pybind11_add_module(order_book bindings/python/order_book.cpp)
target_include_directories(order_book PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/src)
target_link_libraries(order_book PRIVATE dxcore)
set_target_properties(order_book PROPERTIES CXX_STANDARD 17)

install(TARGETS order_book DESTINATION dxcore)
