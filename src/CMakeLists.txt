add_library(dxcore STATIC
        core/order.cpp
        core/price_level.cpp
        core/order_book.cpp
)

target_include_directories(dxcore PUBLIC ${CMAKE_CURRENT_SOURCE_DIR})
target_compile_features(dxcore PUBLIC cxx_std_17)
set_target_properties(dxcore PROPERTIES POSITION_INDEPENDENT_CODE ON)

find_package(Boost REQUIRED CONFIG)
target_link_libraries(dxcore PUBLIC Boost::boost)