#include "price_level.h"
#include "transaction.h"

#include <stdexcept>


PriceLevel::PriceLevel(double price)
    : price(price), totalQuantity(0), index() {}

void PriceLevel::addOrder(Order order) {
    totalQuantity += order.getRemaining();
    orders.push_back(order);
    auto it = std::prev(orders.end());
    index.emplace(it->getId(), it);
}

void PriceLevel::cancelOrder(const Order::Id& id) {
    auto mapIt = index.find(id);
    if (mapIt == index.end())
        throw std::invalid_argument("order not found");

    auto listIt = mapIt->second;
    totalQuantity -= listIt->getRemaining();
    listIt->cancel();
    orders.erase(listIt);
    index.erase(mapIt);
}

std::vector<Transaction> PriceLevel::fill(Order& aggressor) {
    std::vector<Transaction> transactions;
    uint64_t remaining = aggressor.getRemaining();

    while (remaining > 0 && !orders.empty()) {
        Order& passive    = orders.front();
        uint64_t amount   = std::min(remaining, passive.getRemaining());

        passive.fill(amount);
        aggressor.fill(amount);

        remaining     -= amount;
        totalQuantity -= amount;

        const Order::Id& buyId  = aggressor.side == Side::Buy
                                    ? aggressor.getId()
                                    : passive.getId();
        const Order::Id& sellId = aggressor.side == Side::Sell
                                    ? aggressor.getId()
                                    : passive.getId();

        transactions.push_back({ buyId, sellId, price, amount });

        if (passive.status == OrderStatus::Filled) {
            index.erase(passive.getId());
            orders.pop_front();
        }

    }

    return transactions;
}

double   PriceLevel::getPrice()         const { return price; }
uint64_t PriceLevel::getTotalQuantity() const { return totalQuantity; }
bool     PriceLevel::isEmpty()          const { return orders.empty(); }
size_t   PriceLevel::getOrderCount()    const { return orders.size(); }
