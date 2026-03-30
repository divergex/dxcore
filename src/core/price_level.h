#ifndef PRICE_LEVEL_H
#define PRICE_LEVEL_H

#include <list>
#include <unordered_map>
#include <vector>

#include "transaction.h"
#include "order.h"

class PriceLevel {
public:
    explicit PriceLevel(double price);

    void                  addOrder(Order order);
    void                  cancelOrder(const Order::Id& id);
    std::vector<Transaction> fill(Order& aggressor);

    double   getPrice()         const;
    uint64_t getTotalQuantity() const;
    bool     isEmpty()          const;
    size_t   getOrderCount()    const;

private:
    using OrderList = std::list<Order>;
    using OrderMap  = std::unordered_map<Order::Id, OrderList::iterator>;

    double    price;
    uint64_t  totalQuantity;
    OrderList orders;
    OrderMap  index;
};

#endif // PRICE_LEVEL_H
