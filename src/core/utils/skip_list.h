#ifndef DXCORE_SKIP_LIST
#define DXCORE_SKIP_LIST

#include <atomic>

namespace dxcore {

template <typename K, typename T>
class SkipList {
public:
    struct Node {
        K key;
        T data;

        std::atomic<Node*> next;
        std::atomic<Node*> down;

        Node(K key, T data = T{}, Node* down = nullptr);
    };
    T nought;

    SkipList(K nought);
    ~SkipList();

    Node* root;

    Node* search(K key);
    void insert(K key, T data);

};

}

#include "skip_list.tpp"

#endif // DXCORE_SKIP_LIST
