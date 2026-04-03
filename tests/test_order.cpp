// order_test.cpp
#include <gtest/gtest.h>

#include "dxcore.h"

TEST(OrderTest, UniqueIds) {
    Order a(OrderType::Limit, Side::Buy, 100.0, 10);
    Order b(OrderType::Limit, Side::Sell, 101.0, 5);
    EXPECT_FALSE(a == b);
}

TEST(OrderTest, PartialFill) {
    Order o(OrderType::Limit, Side::Buy, 100.0, 10);
    o.fill(4);
    EXPECT_EQ(o.status, OrderStatus::PartiallyFilled);
    EXPECT_EQ(o.getRemaining(), 6);
    EXPECT_EQ(o.filled, 4);
}

TEST(OrderTest, FullFill) {
    Order o(OrderType::Limit, Side::Buy, 100.0, 10);
    o.fill(10);
    EXPECT_EQ(o.status, OrderStatus::Filled);
    EXPECT_EQ(o.getRemaining(), 0);
}

TEST(OrderTest, Cancellation) {
    Order o(OrderType::Limit, Side::Buy, 100.0, 10);
    o.cancel();
    EXPECT_EQ(o.status, OrderStatus::Cancelled);
}

TEST(OrderTest, CannotCancelFilled) {
    Order o(OrderType::Limit, Side::Buy, 100.0, 10);
    o.fill(10);
    EXPECT_THROW(o.cancel(), std::logic_error);
}

TEST(OrderTest, OverfillThrows) {
    Order o(OrderType::Limit, Side::Buy, 100.0, 10);
    EXPECT_THROW(o.fill(11), std::invalid_argument);
}
