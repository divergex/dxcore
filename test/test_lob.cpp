#include <catch2/catch_test_macros.hpp>
#include <chrono>
#include <cstdint>

#include "core/utils/skip_list.h"
#include "core/utils/tp_queue.h"
#include "core/orders/order_book.h"

inline uint64_t now() {
    return std::chrono::duration_cast<std::chrono::microseconds>(
               std::chrono::steady_clock::now().time_since_epoch())
        .count();
}

TEST_CASE("Factorials are computed", "[factorial]") {
    auto pl = std::make_shared<dxcore::LockFreeQueue<dxcore::Order>>();
    pl->insert(std::make_shared<dxcore::Order>(1, 1.0)); // ✅ correct type

    dxcore::OrderBook* lob = new dxcore::OrderBook();

    lob->asks.insert(1.0, pl);

    
}
