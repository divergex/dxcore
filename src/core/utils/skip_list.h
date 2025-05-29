#ifndef DXCORE_SKIP_LIST
#define DXCORE_SKIP_LIST

#include <atomic>
#include <memory>

#include "generator.h"

namespace dxcore {

template <typename K, typename T>
class SkipList {
   public:
    struct Node {
        K key;
        std::shared_ptr<T> data;

        std::atomic<Node*> next;
        std::atomic<Node*> down;
        Node(K key, std::shared_ptr<T> data, Node* down = nullptr)
            : key(std::move(key)),
              data(std::move(data)),
              next(nullptr),
              down(down) {}

        Node(K key, T&& data_val, Node* down = nullptr)
            : key(std::move(key)),
              data(std::make_shared<T>(std::move(data_val))),
              next(nullptr),
              down(down) {}

        Node(K key, const T& data_val, Node* down = nullptr)
            : key(std::move(key)),
              data(std::make_shared<T>(data_val)),
              next(nullptr),
              down(down) {}

        Node(K key, Node* down = nullptr)
            : key(std::move(key)),
              data(std::make_shared<T>()),
              next(nullptr),
              down(down) {}
    };

    K nought;

    SkipList(K nought);
    ~SkipList();

    Node* root;

    Node* search(K key);

    dxcore::Generator<SkipList<K, T>::Node*> inorder();

    template <typename U>
    void insert(K key, U&& data);
};
}  // namespace dxcore

#include "skip_list.tpp"

#endif  // DXCORE_SKIP_LIST
