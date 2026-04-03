// order.cpp
#include <boost/uuid/uuid_generators.hpp>

#include "order.h"

#include <stdexcept>

static boost::uuids::random_generator gen;

Order::Order(OrderType type, Side side, double price, uint64_t quantity)
    : type(type)
    , side(side)
    , price(price)
    , quantity(quantity)
    , filled(0)
    , status(OrderStatus::New)
    , id(gen())
{}

void Order::fill(uint64_t amount) {
    if (amount > getRemaining())
        throw std::invalid_argument("fill amount exceeds remaining quantity");
    filled += amount;
    status = (filled == quantity) ? OrderStatus::Filled : OrderStatus::PartiallyFilled;
}

void Order::cancel() {
    if (status == OrderStatus::Filled)
        throw std::logic_error("cannot cancel a filled order");
    status = OrderStatus::Cancelled;
}