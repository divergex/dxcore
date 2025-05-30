#ifndef DXCORE_LOB
#define DXCORE_LOB

#include "core/utils/tp_queue.h"
#include "core/utils/skip_list.h"

namespace dxcore {

class Order {
   public:
    int quantity;
    float price;

    Order(int quantity, float price) : quantity(quantity), price(price) {};
};

class OrderBook {
    public:
        SkipList<float, LockFreeQueue<Order>> asks;
        SkipList<float, LockFreeQueue<Order>> bids;

    OrderBook() : asks(-1), bids(-1) {};
};

}  // namespace dxcore

#endif  // DXCORE_LOB
