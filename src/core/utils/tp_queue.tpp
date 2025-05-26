#include <utility>

namespace dxcore {

template <typename T>
LockFreeQueue<T>::Node::Node(T val, uint64_t ts)
    : data(std::move(val)), timestamp(ts), next(nullptr), prev(nullptr) {}


template <typename T>
LockFreeQueue<T>::LockFreeQueue() {
    Node* dummy = new Node(T{}, 0);
    head.store(dummy);
    tail.store(dummy);
}


template <typename T>
LockFreeQueue<T>::~LockFreeQueue() {
    while (pop());
    delete head.load();
}


template <typename T>
typename LockFreeQueue<T>::Node* LockFreeQueue<T>::insert(T value, uint64_t timestamp) {
    Node* newNode = new Node(std::move(value), timestamp);

    while (true) {
        Node* curTail = tail.load(std::memory_order_acquire);

        Node* pos = curTail;
        while (pos != nullptr && pos->timestamp > timestamp) {
            pos = pos->prev.load(std::memory_order_acquire);
        }

        if (pos == nullptr) {
            pos = head.load(std::memory_order_acquire);
        }

        Node* nextNode = pos->next.load(std::memory_order_acquire);

        newNode->prev.store(pos, std::memory_order_relaxed);
        newNode->next.store(nextNode, std::memory_order_relaxed);

        if (pos->next.compare_exchange_weak(nextNode, newNode,
                                            std::memory_order_release,
                                            std::memory_order_relaxed)) {
            if (nextNode != nullptr) {
                Node* expectedPrev = pos;
                while (!nextNode->prev.compare_exchange_weak(expectedPrev, newNode,
                                                             std::memory_order_release,
                                                             std::memory_order_relaxed)) {
                    expectedPrev = pos;
                }
            } else {
                Node* expectedTail = pos;
                tail.compare_exchange_weak(expectedTail, newNode,
                                           std::memory_order_release,
                                           std::memory_order_relaxed);
            }
            return newNode;
        }
    }
}


template <typename T>
bool LockFreeQueue<T>::remove(Node* node) {
    if (node == nullptr || node == head.load(std::memory_order_acquire)) {
        // Cannot remove dummy head or nullptr
        return false;
    }

    while (true) {
        Node* prevNode = node->prev.load(std::memory_order_acquire);
        Node* nextNode = node->next.load(std::memory_order_acquire);

        if (prevNode == nullptr) {
            // Node is detached or already removed
            return false;
        }

        // Attempt to unlink node from prevNode->next
        if (!prevNode->next.compare_exchange_weak(node, nextNode,
                                                  std::memory_order_acq_rel,
                                                  std::memory_order_acquire)) {
            // Failed, maybe prevNode->next changed, retry
            continue;
        }

        // Successfully unlinked from prevNode, now unlink from nextNode->prev if nextNode exists
        if (nextNode != nullptr) {
            Node* expectedPrev = node;
            while (!nextNode->prev.compare_exchange_weak(expectedPrev, prevNode,
                                                         std::memory_order_acq_rel,
                                                         std::memory_order_acquire)) {
                if (expectedPrev != node) {
                    // Someone else already updated nextNode->prev, done
                    break;
                }
            }
        } else {
            // We removed the tail node, try to update tail pointer
            Node* expectedTail = node;
            tail.compare_exchange_weak(expectedTail, prevNode,
                                       std::memory_order_acq_rel,
                                       std::memory_order_acquire);
        }

        // Finally mark node as removed by setting its prev to nullptr
        node->prev.store(nullptr, std::memory_order_release);
        node->next.store(nullptr, std::memory_order_release);

        return true;
    }
}


template <typename T>
std::unique_ptr<T> LockFreeQueue<T>::pop() {
    while (true) {
        Node* currHead = head.load(std::memory_order_acquire);
        Node* firstNode = currHead->next.load(std::memory_order_acquire);

        if (firstNode == nullptr) {
            return nullptr;
        }

        if (head.compare_exchange_weak(currHead, firstNode,
                                       std::memory_order_release,
                                       std::memory_order_relaxed)) {
            firstNode->prev.store(nullptr, std::memory_order_relaxed);
            T* value = new T(std::move(firstNode->data));
            delete currHead;
            return std::unique_ptr<T>(value);
        }
    }
}

}

