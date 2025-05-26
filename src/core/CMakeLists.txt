# Create a static library from core sources
add_library(core
    utils/skip_list.cpp
    utils/tp_queue.h
)

# Add headers for consumers
target_include_directories(core PUBLIC ${CMAKE_SOURCE_DIR}/src)
message("SOURCE DIR: ${CMAKE_SOURCE_DIR}")

