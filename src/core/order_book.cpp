#include "order_book.h"

#include <cmath>
#include <limits>
#include <stdexcept>

OrderBook::OrderBook(Instrument instrument, double tickSize)
    : instrument(std::move(instrument))
    , tickSize(tickSize)
{
}

double OrderBook::snapToTick(double price) const {
    return std::round(price / tickSize) * tickSize;
}

double OrderBook::getBestBid() const {
    if (bids.empty())
        return std::numeric_limits<double>::quiet_NaN();
    return bids.begin()->first;
}

double OrderBook::getBestAsk() const {
    if (asks.empty())
        return std::numeric_limits<double>::quiet_NaN();
    return asks.begin()->first;
}

double OrderBook::getMidPrice() const {
    if (bids.empty() || asks.empty())
        return std::numeric_limits<double>::quiet_NaN();
    return (getBestBid() + getBestAsk()) / 2.0;
}

size_t OrderBook::getLevelCount() const {
    return bids.size() + asks.size();
}

void OrderBook::pruneEmptyLevels(double midPrice, double threshold) {
    for (auto it = bids.begin(); it != bids.end(); ) {
        if (it->second.isEmpty() && std::abs(it->first - midPrice) > threshold)
            it = bids.erase(it);
        else
            ++it;
    }
    for (auto it = asks.begin(); it != asks.end(); ) {
        if (it->second.isEmpty() && std::abs(it->first - midPrice) > threshold)
            it = asks.erase(it);
        else
            ++it;
    }
}

std::vector<Transaction> OrderBook::matchAgainstAsks(Order& aggressor) {
    std::vector<Transaction> all;
    double aggressorPrice = aggressor.price;

    for (auto it = asks.begin();
         it != asks.end() && aggressor.getRemaining() > 0 && it->first <= aggressorPrice; ) {

        auto txns = it->second.fill(aggressor);
        for (auto& t : txns) {
            orderIndex.erase(t.sellOrderId);
            all.push_back(t);
        }

        if (it->second.isEmpty())
            it = asks.erase(it);
        else
            ++it;
    }
    return all;
}

std::vector<Transaction> OrderBook::matchAgainstBids(Order& aggressor) {
    std::vector<Transaction> all;
    double aggressorPrice = aggressor.price;

    for (auto it = bids.begin();
         it != bids.end() && aggressor.getRemaining() > 0 && it->first >= aggressorPrice; ) {

        auto txns = it->second.fill(aggressor);
        for (auto& t : txns) {
            orderIndex.erase(t.buyOrderId);
            all.push_back(t);
        }

        if (it->second.isEmpty())
            it = bids.erase(it);
        else
            ++it;
    }
    return all;
}

std::vector<Transaction> OrderBook::insert(Order order) {
    if (orderIndex.count(order.getId()))
        throw std::invalid_argument("duplicate order id");

    double price = snapToTick(order.price);
    std::vector<Transaction> transactions;

    if (order.side == Side::Buy)
        transactions = matchAgainstAsks(order);
    else
        transactions = matchAgainstBids(order);

    // if order has remaining quantity, rest it on the book
    if (order.getRemaining() > 0) {
        Order::Id id = order.getId();

        if (order.side == Side::Buy) {
            if (!bids.count(price))
                bids.emplace(price, PriceLevel(price));
            bids.at(price).addOrder(order);
        } else {
            if (!asks.count(price))
                asks.emplace(price, PriceLevel(price));
            asks.at(price).addOrder(order);
        }

        orderIndex.emplace(id, price);
    }

    // prune levels more than 10% away from mid
    double mid = getMidPrice();
    if (!std::isnan(mid))
        pruneEmptyLevels(mid, mid * 0.10);

    return transactions;
}

void OrderBook::cancel(const Order::Id& id) {
    auto idxIt = orderIndex.find(id);
    if (idxIt == orderIndex.end())
        throw std::invalid_argument("order not found");

    double price = idxIt->second;

    // check bids first then asks
    auto bidIt = bids.find(price);
    if (bidIt != bids.end()) {
        bidIt->second.cancelOrder(id);
        if (bidIt->second.isEmpty())
            bids.erase(bidIt);
        orderIndex.erase(idxIt);
        return;
    }

    auto askIt = asks.find(price);
    if (askIt != asks.end()) {
        askIt->second.cancelOrder(id);
        if (askIt->second.isEmpty())
            asks.erase(askIt);
        orderIndex.erase(idxIt);
        return;
    }

    throw std::logic_error("order found in index but not in book");
}
