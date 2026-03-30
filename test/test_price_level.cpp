#include <gtest/gtest.h>

#include "dxcore.h"

class PriceLevelTest : public testing::Test {
   protected:
    PriceLevel level{100.0};

    static Order makeBuy(double price, uint64_t quantity) {
        return {OrderType::Limit, Side::Buy, price, quantity};
    }

    static Order makeSell(double price, uint64_t quantity) {
        return {OrderType::Limit, Side::Sell, price, quantity};
    }
};

TEST_F(PriceLevelTest, AddOrder) {
    level.addOrder(makeBuy(100.0, 10));
    EXPECT_EQ(level.getOrderCount(), 1);
    EXPECT_EQ(level.getTotalQuantity(), 10);
    EXPECT_FALSE(level.isEmpty());
}

TEST_F(PriceLevelTest, AddMultipleOrders) {
    level.addOrder(makeBuy(100.0, 10));
    level.addOrder(makeBuy(100.0, 20));
    EXPECT_EQ(level.getOrderCount(), 2);
    EXPECT_EQ(level.getTotalQuantity(), 30);
}

TEST_F(PriceLevelTest, CancelOrder) {
    Order o = makeBuy(100.0, 10);
    Order::Id id = o.getId();
    level.addOrder(o);

    level.cancelOrder(id);
    EXPECT_EQ(level.getOrderCount(), 0);
    EXPECT_EQ(level.getTotalQuantity(), 0);
    EXPECT_TRUE(level.isEmpty());
}

TEST_F(PriceLevelTest, CancelNonExistentOrderThrows) {
    Order o = makeBuy(100.0, 10);
    Order::Id id = o.getId();
    EXPECT_THROW(level.cancelOrder(id), std::invalid_argument);
}

TEST_F(PriceLevelTest, CancelOneOfMany) {
    Order a = makeBuy(100.0, 10);
    Order b = makeBuy(100.0, 20);
    Order::Id idA = a.getId();

    level.addOrder(a);
    level.addOrder(b);
    level.cancelOrder(idA);

    EXPECT_EQ(level.getOrderCount(), 1);
    EXPECT_EQ(level.getTotalQuantity(), 20);
}

TEST_F(PriceLevelTest, PartialFill) {
    level.addOrder(makeBuy(100.0, 10));
    Order aggressor = makeSell(100.0, 4);
    auto txns = level.fill(aggressor);

    EXPECT_EQ(txns.size(), 1);
    EXPECT_EQ(txns[0].quantity, 4);
    EXPECT_EQ(txns[0].price, 100.0);
    EXPECT_EQ(level.getTotalQuantity(), 6);
    EXPECT_EQ(level.getOrderCount(), 1);
}

TEST_F(PriceLevelTest, FullFill) {
    level.addOrder(makeBuy(100.0, 10));
    Order aggressor = makeSell(100.0, 10);
    auto txns = level.fill(aggressor);

    EXPECT_EQ(txns.size(), 1);
    EXPECT_EQ(txns[0].quantity, 10);
    EXPECT_TRUE(level.isEmpty());
}

TEST_F(PriceLevelTest, FillAcrossMultipleOrders) {
    level.addOrder(makeBuy(100.0, 10));
    level.addOrder(makeBuy(100.0, 10));
    Order aggressor = makeSell(100.0, 15);
    auto txns = level.fill(aggressor);

    EXPECT_EQ(txns.size(), 2);
    EXPECT_EQ(txns[0].quantity, 10);
    EXPECT_EQ(txns[1].quantity, 5);
    EXPECT_EQ(level.getTotalQuantity(), 5);
    EXPECT_EQ(level.getOrderCount(), 1);
}

TEST_F(PriceLevelTest, FillMoreThanAvailable) {
    level.addOrder(makeBuy(100.0, 10));
    Order aggressor = makeSell(100.0, 20);
    auto txns = level.fill(aggressor);

    EXPECT_EQ(txns.size(), 1);
    EXPECT_EQ(txns[0].quantity, 10);
    EXPECT_TRUE(level.isEmpty());
}

TEST_F(PriceLevelTest, TransactionBuySellIds) {
    Order buyer = makeBuy(100.0, 10);
    Order seller = makeSell(100.0, 10);
    Order::Id buyId = buyer.getId();
    Order::Id sellId = seller.getId();

    level.addOrder(buyer);
    auto txns = level.fill(seller);

    EXPECT_EQ(txns.size(), 1);
    EXPECT_EQ(txns[0].buyOrderId, buyId);
    EXPECT_EQ(txns[0].sellOrderId, sellId);
}

TEST_F(PriceLevelTest, FifoOrdering) {
    Order a = makeBuy(100.0, 5);
    Order b = makeBuy(100.0, 10);
    Order::Id idA = a.getId();

    level.addOrder(a);
    level.addOrder(b);

    Order aggressor = makeSell(100.0, 5);
    level.fill(aggressor);

    EXPECT_THROW(level.cancelOrder(idA), std::invalid_argument);
    EXPECT_EQ(level.getOrderCount(), 1);
    EXPECT_EQ(level.getTotalQuantity(), 10);
}
