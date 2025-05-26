import random

class Node:
    def __init__(self, key, value=None, down=None):
        self.value = value
        self.key = key
        self.next_node = None
        self.down = down

class Skip:
    def __init__(self):
        self.root = Node(-1)
        self.levels = 0

    def search(self, key):
        node = self.root
        while node:
            while node.next_node and key >= node.next_node.key:
                node = node.next_node
            if key == node.key:
                while node.down is not None:
                    node = node.down
                break
            else:
                node = node.down
        return node  # None

    def inorder(self):
        if not (node := self.root):
            return
        while node.down is not None:
            node = node.down
        node = node.next_node
        while node:
            yield node
            node = node.next_node
        return

    def insert(self, value, key):
        node = self.root
        previous = []
        while node:
            while node.next_node and node.next_node.key < key:
                node = node.next_node
            previous.append(node)
            node = node.down
        i = 0
        while i < len(previous):
            node = Node(key, value, node)
            prev = previous[-(i + 1)]
            node.next_node = prev.next_node
            prev.next_node = node
            if random.random() < 0.5:
                break
            i += 1
        if i == len(previous):
            self.root = Node(-1, down=self.root)
            self.root.next_node = Node(key, value, node)


s = Skip()
s.insert("a", 0)
s.insert("c", 2)
s.insert("b", 1)
print([node.value for node in s.inorder()])
