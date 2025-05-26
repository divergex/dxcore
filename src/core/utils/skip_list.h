#ifndef DXCORE_SKIP_LIST
#define DXCORE_SKIP_LIST

namespace dxcore {

template <typename T>
class SkipList {
public:
    struct Node {
        T data;
        int key;

        std::atomic<Node*> next;
        std::atomic<Node*> down;

        Node(T data, int key, std::atomic<Node*> down);
    };


};

}

#endif // DXCORE_SKIP_LIST
