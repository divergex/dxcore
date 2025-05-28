# Create a static library from core sources
add_library(core
    utils/skip_list.h
    utils/tp_queue.h
    core.cpp
)

# Add headers for consumers
target_include_directories(core PUBLIC ${CMAKE_SOURCE_DIR}/src)
message("SOURCE DIR: ${CMAKE_SOURCE_DIR}")

