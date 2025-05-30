#pragma once

#include <atomic>
#include <cstdlib>
#include <iostream>
#include <memory>
#include <vector>

#include "generator.h"
#include "skip_list.h"

namespace dxcore {


template <typename K, typename T>
SkipList<K, T>::SkipList(K nought) : nought(nought) {
    this->root = new Node(nought);
}

template <typename K, typename T>
SkipList<K, T>::~SkipList() {
    delete this->root;
}

template <typename K, typename T>
typename std::shared_ptr<T> SkipList<K, T>::search(K key) {
    Node* node = this->root;
    Node* next;
    while (node != nullptr) {
        next = node->next.load();
        while (next != nullptr && next->key <= key) {
            node = next;
            next = node->next.load();
        }
        if (node->key == key) {
            Node* down = node->down.load();
            while (down != nullptr) {
                node = down;
                down = node->down.load();
            }
            return node->data;
        } else {
            node = node->down.load();
        }
    }
    return node->data;
}

template <typename K, typename T>
Generator<typename SkipList<K, T>::Node*> SkipList<K, T>::inorder() {
    Node* node = this->root;
    if (!node) co_return;
    while (node->down) node = node->down;
    node = node->next.load();
    while (node) {
        co_yield node;
        node = node->next.load();
    }
}

template <typename K, typename T>
template <typename U>
void SkipList<K, T>::insert(K key, U&& data) {
    Node* node = this->root;
    std::vector<Node*> previous;

    Node* next;
    while (node) {
        next = node->next.load();
        while (next && next->key < key) {
            node = next;
            next = node->next.load();
        }
        previous.push_back(node);
        node = node->down.load();
    }
    int promotions = 0;

    Node* prev;
    while (promotions < static_cast<int>(previous.size())) {
        // Forward data here for move/copy efficiency
        node = new Node(key, std::forward<U>(data), node);
        prev = previous.at(previous.size() - promotions - 1);
        node->next = prev->next.load();
        prev->next = node;

        if (rand() < RAND_MAX / 2) break; // use RAND_MAX for correct probability
        promotions++;
    }
    if (promotions == static_cast<int>(previous.size())) {
        this->root = new Node(this->nought, this->root);
        this->root->next = new Node(key, std::forward<U>(data), node);
    }
}

}  // namespace dxcore

