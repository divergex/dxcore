#include <algorithm>
#include <catch2/catch_test_macros.hpp>
#include "core/utils/tp_queue.h"
#include "core/utils/skip_list.h"
#include <thread>
#include <chrono>
#include <iostream>

unsigned int Factorial( unsigned int number ) {
    return number <= 1 ? number : Factorial(number-1)*number;
};

TEST_CASE( "Factorials are computed", "[factorial]" ) {
    REQUIRE( Factorial(1) == 1 );
    REQUIRE( Factorial(2) == 2 );
    REQUIRE( Factorial(3) == 6 );
    REQUIRE( Factorial(10) == 3628800 );
    
    dxcore::LockFreeQueue<int> q;
    dxcore::SkipList<int, int> sk(-1);

    std::cout << sk.search(4)->key;

    auto now = []() -> uint64_t {
        return std::chrono::duration_cast<std::chrono::microseconds>(
                   std::chrono::steady_clock::now().time_since_epoch())
            .count();
    };
    dxcore::LockFreeQueue<int>::Node* invalid = q.insert(-1, 0);

    std::thread t1([&]() {
        for (int i = 0; i < 10; i += 2) {
            uint64_t ts = now() + i * 10;
            q.insert(i, ts);
        }
    });

    std::thread t2([&]() {
        for (int i = 1; i < 10; i += 2) {
            uint64_t ts = now();
            q.insert(i, ts);
        }
    });

    std::thread t3([&]() {
        for (int i = 0; i < 10; i++) {
            while (true) {
                auto val = q.pop();
                if (val) {
                    std::cout << "Popped: " << *val << std::endl;
                    break;
                }
                std::this_thread::yield();
            }
        }
    });

    // bool success = q.remove(invalid);
    // std::cout << success << std::endl;

    t1.join();
    t2.join();
    t3.join();
}
