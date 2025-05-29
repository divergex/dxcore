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
        T data;
        uint64_t timestamp;
        std::atomic<Node*> next;
        std::atomic<Node*> prev;

        template <typename U>
        Node(U&& data, uint64_t ts = {});
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
    LockFreeQueue<T>::Node* insert(U&& data, uint64_t timestamp);
    template <typename U>
    LockFreeQueue<T>::Node* insert(U&& data);
    bool remove(LockFreeQueue<T>::Node* node);
    std::unique_ptr<T> pop();
};

}  // namespace dxcore

#include "tp_queue.tpp"

#endif  // DXCORE_TP_QUEUE
