#ifndef DXCORE_TP_QUEUE
#define DXCORE_TP_QUEUE

#include <atomic>
#include <memory>
#include <cstdint>

namespace dxcore {

template <typename T>
class LockFreeQueue {
public:
    struct Node {
        T data;
        uint64_t timestamp;
        std::atomic<Node*> next;
        std::atomic<Node*> prev;

        Node(T val, uint64_t ts);
    };

    std::atomic<Node*> head;
    std::atomic<Node*> tail;

public:
    LockFreeQueue();
    ~LockFreeQueue();

    LockFreeQueue<T>::Node* insert(T value, uint64_t timestamp);
    bool remove(LockFreeQueue<T>::Node* node);
    std::unique_ptr<T> pop();
};

}


#include "tp_queue.tpp"

#endif // DXCORE_TP_QUEUE

