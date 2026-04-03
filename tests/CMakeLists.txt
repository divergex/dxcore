find_package(GTest REQUIRED)

enable_testing()

add_executable(dxcore-tests
        test_order.cpp
        test_price_level.cpp
        test_order_book.cpp
)

target_link_libraries(dxcore-tests PRIVATE
        dxcore
        GTest::gtest_main
)

include(GoogleTest)
gtest_discover_tests(dxcore-tests)