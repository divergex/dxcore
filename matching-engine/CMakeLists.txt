cmake_minimum_required(VERSION 4.0)
project(matching_engine)

FetchContent_Declare(
        googletest
        GIT_REPOSITORY https://github.com/google/googletest.git
        GIT_TAG        703bd9caab50b139428cea1aaff9974ebee5742e # release-1.10.0
)

FetchContent_MakeAvailable(googletest)

set(CMAKE_CXX_STANDARD 20)

add_executable(matching_engine main.cpp
        src/price_level.cpp
        src/price_level.h
        src/order.cpp
        src/order.h
        src/instrument.h)
