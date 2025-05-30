#ifndef DXCORE_TP_QUEUE
#define DXCORE_TP_QUEUE

#include <atomic>
#include <cstdint>
#include <memory>

namespace dxcore {

template <typename T>
class LockFreeQueue {
   public:
    struct Node {
        std::shared_ptr<T> data;
        uint64_t timestamp;
        std::atomic<Node*> next;
        std::atomic<Node*> prev;

        Node() : timestamp(0), data(nullptr), next(nullptr), prev(nullptr) {}

        explicit Node(uint64_t ts)
            : timestamp(ts), data(nullptr), next(nullptr), prev(nullptr) {}

        template <typename U>
        Node(U&& d,
             std::enable_if_t<
                 std::is_same_v<std::decay_t<U>, std::shared_ptr<T>>, int> = 0)
            : timestamp(0),
              data(std::forward<U>(d)),
              next(nullptr),
              prev(nullptr) {}

        template <typename U>
        Node(uint64_t ts, U&& d,
             std::enable_if_t<
                 std::is_same_v<std::decay_t<U>, std::shared_ptr<T>>, int> = 0)
            : timestamp(ts),
              data(std::forward<U>(d)),
              next(nullptr),
              prev(nullptr) {}
    };

    std::atomic<Node*> head;
    std::atomic<Node*> tail;

   public:
    LockFreeQueue();
    ~LockFreeQueue();

    LockFreeQueue(const LockFreeQueue&) = delete;
    LockFreeQueue& operator=(const LockFreeQueue&) = delete;

    LockFreeQueue(LockFreeQueue&& other) noexcept;
    LockFreeQueue& operator=(LockFreeQueue&& other) noexcept;

    template <typename U>
    Node* insert(U&& data, uint64_t timestamp);
    template <typename U>
    Node* insert(U&& data);

    bool remove(LockFreeQueue<T>::Node* node);
    std::shared_ptr<T> pop();
};

}  // namespace dxcore

#include "tp_queue.tpp"

#endif  // DXCORE_TP_QUEUE
