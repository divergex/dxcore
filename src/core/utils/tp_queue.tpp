#pragma once

#include <utility>
#include <chrono>

#include "tp_queue.h"

namespace dxcore {


template <typename T>
LockFreeQueue<T>::LockFreeQueue() {
    Node* dummy = new Node();
    head.store(dummy);
    tail.store(dummy);
}

template <typename T>
LockFreeQueue<T>::~LockFreeQueue() {
    while (pop());
    delete head.load();
}

template <typename T>
template <typename U>
typename LockFreeQueue<T>::Node* LockFreeQueue<T>::insert(U&& data,
                                                          uint64_t timestamp) {
    Node* newNode = new Node(timestamp, std::forward<U>(data));

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
                while (!nextNode->prev.compare_exchange_weak(
                    expectedPrev, newNode, std::memory_order_release,
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
template <typename U>
typename LockFreeQueue<T>::Node* LockFreeQueue<T>::insert(U&& data) {
    uint64_t now = std::chrono::duration_cast<std::chrono::milliseconds>(
                       std::chrono::system_clock::now().time_since_epoch())
                       .count();
    return insert(std::forward<U>(data), now);
}

template <typename T>
bool LockFreeQueue<T>::remove(Node* node) {
    if (node == nullptr || node == head.load(std::memory_order_acquire)) {
        return false;
    }

    while (true) {
        Node* prevNode = node->prev.load(std::memory_order_acquire);
        Node* nextNode = node->next.load(std::memory_order_acquire);

        if (prevNode == nullptr) {
            return false;
        }

        if (!prevNode->next.compare_exchange_weak(node, nextNode,
                                                  std::memory_order_acq_rel,
                                                  std::memory_order_acquire)) {
            continue;
        }

        if (nextNode != nullptr) {
            Node* expectedPrev = node;
            while (!nextNode->prev.compare_exchange_weak(
                expectedPrev, prevNode, std::memory_order_acq_rel,
                std::memory_order_acquire)) {
                if (expectedPrev != node) {
                    break;
                }
            }
        } else {
            Node* expectedTail = node;
            tail.compare_exchange_weak(expectedTail, prevNode,
                                       std::memory_order_acq_rel,
                                       std::memory_order_acquire);
        }

        node->prev.store(nullptr, std::memory_order_release);
        node->next.store(nullptr, std::memory_order_release);

        return true;
    }
}

template <typename T>
std::shared_ptr<T> LockFreeQueue<T>::pop() {
    while (true) {
        Node* currHead = head.load(std::memory_order_acquire);
        
        if (!currHead)
            return nullptr;

        Node* firstNode = currHead->next.load(std::memory_order_acquire);

        if (firstNode == nullptr) {
            return nullptr;
        }

        if (head.compare_exchange_weak(currHead, firstNode,
                                       std::memory_order_release,
                                       std::memory_order_relaxed)) {
            firstNode->prev.store(nullptr, std::memory_order_relaxed);
            delete currHead;
            return firstNode->data;
        }
    }
}

template <typename T>
LockFreeQueue<T>::LockFreeQueue(LockFreeQueue&& other) noexcept
    : head(other.head.load(std::memory_order_acquire)),
      tail(other.tail.load(std::memory_order_acquire)) {
    other.head.store(nullptr, std::memory_order_release);
    other.tail.store(nullptr, std::memory_order_release);
}

template <typename T>
LockFreeQueue<T>& LockFreeQueue<T>::operator=(LockFreeQueue&& other) noexcept {
    if (this != &other) {
        while (pop());
        delete head.load();

        head.store(other.head.load(std::memory_order_acquire));
        tail.store(other.tail.load(std::memory_order_acquire));

        other.head.store(nullptr, std::memory_order_release);
        other.tail.store(nullptr, std::memory_order_release);
    }
    return *this;
}

}  // namespace dxcore
