#ifndef PRICE_LEVEL_H
#define PRICE_LEVEL_H

#include <boost/container_hash/hash.hpp>

#include <list>
#include <unordered_map>
#include <vector>

#include "order.h"
#include "transaction.h"

class PriceLevel {
   public:
    explicit PriceLevel(double price);

    void addOrder(Order order);
    void cancelOrder(const Order::Id& id);
    std::vector<Transaction> fill(Order& aggressor);

    double getPrice() const;
    uint64_t getTotalQuantity() const;
    bool isEmpty() const;
    size_t getOrderCount() const;

   private:
    struct UuidHash {
        size_t operator()(const boost::uuids::uuid& id) const {
            return boost::hash<boost::uuids::uuid>()(id);
        }
    };

    using OrderList = std::list<Order>;
    using OrderMap  = std::unordered_map<Order::Id, OrderList::iterator, UuidHash>;

    double    price;
    uint64_t  totalQuantity;
    OrderList orders;
    OrderMap  index;
};

#endif  // PRICE_LEVEL_H
