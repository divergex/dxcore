#include <atomic>
#include <iostream>

namespace dxcore {

template <typename K, typename T>
SkipList<K, T>::Node::Node(K key, T data, Node* down)
    : key(std::move(key)),
      data(std::move(data)),
      next(nullptr),
      down(std::move(down)){};

template <typename K, typename T>
SkipList<K, T>::SkipList(K nought)
    : nought(nought)
{
    this->root = new Node(nought);
};

template <typename K, typename T>
SkipList<K, T>::~SkipList() {
    delete this->root;
};

template <typename K, typename T>
typename SkipList<K, T>::Node* SkipList<K, T>::search(K key) {
    Node* node = this->root;
    return node;
};

}  // namespace dxcore
