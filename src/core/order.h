#ifndef ORDER_H
#define ORDER_H

#include <boost/uuid/uuid.hpp>
#include <cstdint>

enum class Side { Buy, Sell };
enum class OrderType { Market, Limit };
enum class OrderStatus { New, PartiallyFilled, Filled, Cancelled };

class Order {
public:
    using Id = boost::uuids::uuid;

    Order(OrderType type, Side side, double price, uint64_t quantity);

    bool operator==(const Order& other) const { return id == other.id; }
    bool operator<(const Order& other) const  { return id < other.id; }

    [[nodiscard]] Id          getId()         const { return id; }
    [[nodiscard]] uint64_t    getRemaining()  const { return quantity - filled; }

    void fill(uint64_t amount);
    void cancel();

    const OrderType   type;
    const Side        side;
    const double      price;
    const uint64_t    quantity;

    uint64_t    filled;
    OrderStatus status;

private:
    Id          id;
};

#endif // ORDER_H
