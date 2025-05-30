#include <algorithm>
#include <catch2/catch_test_macros.hpp>
#include <chrono>
#include <iostream>
#include <thread>

#include "core/utils/skip_list.h"
#include "core/utils/tp_queue.h"


TEST_CASE("Factorials are computed", "[factorial]") {
    auto q = std::make_shared<dxcore::LockFreeQueue<int>>();
    q->insert(1);
    dxcore::SkipList<int, dxcore::LockFreeQueue<int>> sk(-1);
    
    sk.insert(1, q);

    // auto gen = sk.inorder();
    // while (!gen.done()) {
    //     auto node = gen.next();
    //     if (node) {
    //         std::cout << node->key << ": " << node->data->pop() << '\n';
    //     }
    // }
    auto now = []() -> uint64_t {
        return std::chrono::duration_cast<std::chrono::microseconds>(
                   std::chrono::steady_clock::now().time_since_epoch())
            .count();
    };

    q->insert(2, now());
    // dxcore::LockFreeQueue<int>::Node* invalid = q.insert(-1, 0);
    std::thread t1([&]() {
        for (int i = 0; i < 10; i += 2) {
            int64_t ts = now() + i * 10;
            q->insert(i, ts);
        }
    });

    std::thread t2([&]() {
        for (int i = 1; i < 10; i += 2) {
            uint64_t ts = now();
            q->insert(i, ts);
        }
    });

    std::thread t3([&]() {
        q = sk.search(1);
        for (int i = 0; i < 10; i++) {
            while (true) {
                auto val = q->pop();
                if (val) {
                    std::cout << "Popped: " << *val << std::endl;
                    break;
                }
                std::this_thread::yield();
            }
        }
    });

    t1.join();
    t2.join();
    t3.join();
}
