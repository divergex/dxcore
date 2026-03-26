//
// Created by rzimmerdev on 2/26/26.
//

#ifndef MATCHING_ENGINE_PRICE_LEVEL_H
#define MATCHING_ENGINE_PRICE_LEVEL_H
#include <list>

#include "order.h"

class PriceLevel
{
public:
    PriceLevel();
    void insert(Order order);

private:
    std::list<Order> queue;
};


#endif //MATCHING_ENGINE_PRICE_LEVEL_H