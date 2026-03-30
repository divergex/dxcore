#include <gtest/gtest.h>
#include <cmath>

#include "dxcore.h"

class OrderBookTest : public testing::Test {
protected:
    std::unique_ptr<OrderBook> book;

    void SetUp() override {
        book = std::make_unique<OrderBook>(Instrument("AAPL"));
    }

    static Order makeBuy(double price, uint64_t quantity) {
        return {OrderType::Limit, Side::Buy, price, quantity};
    }

    static Order makeSell(double price, uint64_t quantity) {
        return {OrderType::Limit, Side::Sell, price, quantity};
    }
};

// ── basic structure ───────────────────────────────────────────────────────────

TEST_F(OrderBookTest, EmptyBookHasNaNMid) {
    EXPECT_TRUE(std::isnan(book->getMidPrice()));
    EXPECT_TRUE(std::isnan(book->getBestBid()));
    EXPECT_TRUE(std::isnan(book->getBestAsk()));
}

TEST_F(OrderBookTest, InsertBidUpdatesBestBid) {
    book->insert(makeBuy(100.0, 10));
    EXPECT_DOUBLE_EQ(book->getBestBid(), 100.0);
}

TEST_F(OrderBookTest, InsertAskUpdatesBestAsk) {
    book->insert(makeSell(101.0, 10));
    EXPECT_DOUBLE_EQ(book->getBestAsk(), 101.0);
}

TEST_F(OrderBookTest, MidPrice) {
    book->insert(makeBuy(100.0, 10));
    book->insert(makeSell(102.0, 10));
    EXPECT_DOUBLE_EQ(book->getMidPrice(), 101.0);
}

TEST_F(OrderBookTest, BestBidIsHighest) {
    book->insert(makeBuy(99.0, 10));
    book->insert(makeBuy(100.0, 10));
    book->insert(makeBuy(98.0, 10));
    EXPECT_DOUBLE_EQ(book->getBestBid(), 100.0);
}

TEST_F(OrderBookTest, BestAskIsLowest) {
    book->insert(makeSell(102.0, 10));
    book->insert(makeSell(101.0, 10));
    book->insert(makeSell(103.0, 10));
    EXPECT_DOUBLE_EQ(book->getBestAsk(), 101.0);
}

// ── tick snapping ─────────────────────────────────────────────────────────────

TEST_F(OrderBookTest, PriceSnappedToTick) {
    book->insert(makeBuy(100.004, 10));
    EXPECT_DOUBLE_EQ(book->getBestBid(), 100.00);

    book->insert(makeSell(100.996, 10));
    EXPECT_DOUBLE_EQ(book->getBestAsk(), 101.00);
}

// ── no match ──────────────────────────────────────────────────────────────────

TEST_F(OrderBookTest, NoMatchWhenSpreadExists) {
    book->insert(makeBuy(100.0, 10));
    auto txns = book->insert(makeSell(101.0, 10));
    EXPECT_TRUE(txns.empty());
    EXPECT_EQ(book->getLevelCount(), 2);
}

// ── matching ──────────────────────────────────────────────────────────────────

TEST_F(OrderBookTest, FullMatch) {
    book->insert(makeBuy(100.0, 10));
    auto txns = book->insert(makeSell(100.0, 10));

    EXPECT_EQ(txns.size(), 1);
    EXPECT_EQ(txns[0].quantity, 10);
    EXPECT_DOUBLE_EQ(txns[0].price, 100.0);
    EXPECT_TRUE(std::isnan(book->getBestBid()));
    EXPECT_EQ(book->getLevelCount(), 0);
}

TEST_F(OrderBookTest, PartialMatchRestsBid) {
    book->insert(makeBuy(100.0, 10));
    auto txns = book->insert(makeSell(100.0, 4));

    EXPECT_EQ(txns.size(), 1);
    EXPECT_EQ(txns[0].quantity, 4);
    EXPECT_DOUBLE_EQ(book->getBestBid(), 100.0);
    EXPECT_EQ(book->getLevelCount(), 1);
}

TEST_F(OrderBookTest, PartialMatchRestsAsk) {
    book->insert(makeSell(100.0, 10));
    auto txns = book->insert(makeBuy(100.0, 4));

    EXPECT_EQ(txns.size(), 1);
    EXPECT_EQ(txns[0].quantity, 4);
    EXPECT_DOUBLE_EQ(book->getBestAsk(), 100.0);
    EXPECT_EQ(book->getLevelCount(), 1);
}

TEST_F(OrderBookTest, SweepMultipleLevels) {
    book->insert(makeSell(100.0, 5));
    book->insert(makeSell(101.0, 5));
    book->insert(makeSell(102.0, 5));

    auto txns = book->insert(makeBuy(102.0, 15));

    EXPECT_EQ(txns.size(), 3);
    EXPECT_EQ(txns[0].quantity, 5);
    EXPECT_EQ(txns[1].quantity, 5);
    EXPECT_EQ(txns[2].quantity, 5);
    EXPECT_TRUE(std::isnan(book->getBestAsk()));
    EXPECT_EQ(book->getLevelCount(), 0);
}

TEST_F(OrderBookTest, SweepMultipleLevelsPartial) {
    book->insert(makeSell(100.0, 5));
    book->insert(makeSell(101.0, 5));
    book->insert(makeSell(102.0, 5));

    auto txns = book->insert(makeBuy(101.0, 12));

    // EXPECT_EQ(txns.size(), 3);
    EXPECT_EQ(txns.size(), 2);
    EXPECT_EQ(txns[0].quantity, 5);  // fully consumed 100.0
    EXPECT_EQ(txns[1].quantity, 5);  // fully consumed 101.0
    EXPECT_EQ(txns[2].quantity, 2);  // partial at 101.0
    EXPECT_DOUBLE_EQ(book->getBestAsk(), 101.0);
    EXPECT_DOUBLE_EQ(book->getBestBid(), 101.0);
}

TEST_F(OrderBookTest, TransactionIdsCorrect) {
    Order buyer  = makeBuy(100.0, 10);
    Order seller = makeSell(100.0, 10);
    Order::Id buyId  = buyer.getId();
    Order::Id sellId = seller.getId();

    book->insert(buyer);
    auto txns = book->insert(seller);

    EXPECT_EQ(txns.size(), 1);
    EXPECT_EQ(txns[0].buyOrderId,  buyId);
    EXPECT_EQ(txns[0].sellOrderId, sellId);
}

// ── cancel ────────────────────────────────────────────────────────────────────

TEST_F(OrderBookTest, CancelBid) {
    Order o = makeBuy(100.0, 10);
    Order::Id id = o.getId();
    book->insert(o);

    book->cancel(id);
    EXPECT_TRUE(std::isnan(book->getBestBid()));
    EXPECT_EQ(book->getLevelCount(), 0);
}

TEST_F(OrderBookTest, CancelAsk) {
    Order o = makeSell(101.0, 10);
    Order::Id id = o.getId();
    book->insert(o);

    book->cancel(id);
    EXPECT_TRUE(std::isnan(book->getBestAsk()));
    EXPECT_EQ(book->getLevelCount(), 0);
}

TEST_F(OrderBookTest, CancelNonExistentThrows) {
    Order o = makeBuy(100.0, 10);
    Order::Id id = o.getId();
    EXPECT_THROW(book->cancel(id), std::invalid_argument);
}

TEST_F(OrderBookTest, CancelOneOfManyAtSameLevel) {
    Order a = makeBuy(100.0, 10);
    Order b = makeBuy(100.0, 20);
    Order::Id idA = a.getId();

    book->insert(a);
    book->insert(b);
    book->cancel(idA);

    EXPECT_DOUBLE_EQ(book->getBestBid(), 100.0);
    EXPECT_EQ(book->getLevelCount(), 1);
}

TEST_F(OrderBookTest, CancelledOrderNotMatchable) {
    Order o = makeBuy(100.0, 10);
    Order::Id id = o.getId();
    book->insert(o);
    book->cancel(id);

    auto txns = book->insert(makeSell(100.0, 10));
    EXPECT_TRUE(txns.empty());
}

// ── duplicate ─────────────────────────────────────────────────────────────────

TEST_F(OrderBookTest, DuplicateOrderIdThrows) {
    Order o = makeBuy(100.0, 10);
    Order copy = o;
    book->insert(o);
    EXPECT_THROW(book->insert(copy), std::invalid_argument);
}
