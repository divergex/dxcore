#ifndef DXCORE_GENERATOR
#define DXCORE_GENERATOR

#include <coroutine>

namespace dxcore {

template<typename T>
struct Generator {
    struct promise_type;
    using handle_type = std::coroutine_handle<promise_type>;

    struct promise_type {
        T current;
        std::suspend_always yield_value(T value) {
            current = value;
            return {};
        }
        std::suspend_always initial_suspend() { return {}; }
        std::suspend_always final_suspend() noexcept { return {}; }
        Generator get_return_object() {
            return Generator{handle_type::from_promise(*this)};
        }
        void return_void() {}
        void unhandled_exception() { std::terminate(); }
    };

    handle_type coro;
    Generator(handle_type h) : coro(h) {}
    ~Generator() { if (coro) coro.destroy(); }

    T next() {
        coro.resume();
        return coro.done() ? nullptr : coro.promise().current;
    }

    bool done() const { return coro.done(); }
};

}

#endif // DXCORE_GENERATOR
