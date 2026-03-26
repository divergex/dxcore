//
// Created by rzimmerdev on 2/26/26.
//

#include "price_level.h"

#include <list>

PriceLevel::PriceLevel()
{
    queue = std::list<Order>();
}

void PriceLevel::insert(const Order order)
{
    queue.push_back(order);
};
