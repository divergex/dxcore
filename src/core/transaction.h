#ifndef TRANSACTION_H
#define TRANSACTION_H

#include "order.h"

struct Transaction {
    Order::Id buyOrderId{};
    Order::Id sellOrderId{};
    double    price{};
    uint64_t  quantity{};
};

#endif // TRANSACTION_H
