use std::collections::{BTreeSet, HashMap};

use crate::trading::Side;

use super::{OrderId, Price};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct LevelId(pub(super) usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct NodeId(pub(super) usize);

#[derive(Debug)]
struct Level {
    price: Price,
    prev: Option<LevelId>,
    next: Option<LevelId>,
    head: Option<NodeId>,
    tail: Option<NodeId>,
    quantity: f64,
    orders: usize,
}

impl Level {
    fn new(price: Price) -> Self {
        Self {
            price,
            prev: None,
            next: None,
            head: None,
            tail: None,
            quantity: 0.0,
            orders: 0,
        }
    }
}

#[derive(Debug)]
struct Node {
    id: OrderId,
    quantity: f64,
    prev: Option<NodeId>,
    next: Option<NodeId>,
}

impl Node {
    fn new(id: OrderId, quantity: f64) -> Self {
        Self {
            id,
            quantity,
            prev: None,
            next: None,
        }
    }
}

/// Existing price levels are O(1) hash lookups. The linked list is ordered
/// best-to-worst, and `best` always points at the first non-empty level.
/// Empty levels are removed immediately.
///
/// A `BTreeSet` supplies the ordered index needed to position a *new* level;
/// that insertion is O(log n) because arbitrary price keys cannot be ordered in
/// O(1) without a bounded tick ladder or an insertion hint.
#[derive(Debug)]
pub(super) struct PriceLevels {
    side: Side,
    by_price: HashMap<Price, LevelId>,
    prices: BTreeSet<Price>,
    levels: Vec<Level>,
    free_levels: Vec<LevelId>,
    nodes: Vec<Node>,
    free_nodes: Vec<NodeId>,
    head: Option<LevelId>,
    tail: Option<LevelId>,
    best: Option<LevelId>,
}

impl PriceLevels {
    pub(super) fn new(side: Side) -> Self {
        Self {
            side,
            by_price: HashMap::new(),
            prices: BTreeSet::new(),
            levels: Vec::new(),
            free_levels: Vec::new(),
            nodes: Vec::new(),
            free_nodes: Vec::new(),
            head: None,
            tail: None,
            best: None,
        }
    }

    pub(super) fn best(&self) -> Option<(Price, LevelId)> {
        self.best.map(|level| (self.levels[level.0].price, level))
    }

    pub(super) fn head(&self, level: LevelId) -> Option<(OrderId, f64)> {
        let head = self.levels.get(level.0)?.head?;
        let node = self.nodes.get(head.0)?;
        Some((node.id, node.quantity))
    }

    pub(super) fn append(&mut self, price: Price, id: OrderId, quantity: f64) -> (LevelId, NodeId) {
        let level = self.insert_level(price);
        let was_empty = self.levels[level.0].orders == 0;
        let node = self.alloc_node(id, quantity);
        let tail = self.levels[level.0].tail;

        self.nodes[node.0].prev = tail;
        if let Some(tail) = tail {
            self.nodes[tail.0].next = Some(node);
        } else {
            self.levels[level.0].head = Some(node);
        }
        self.levels[level.0].tail = Some(node);
        self.levels[level.0].quantity += quantity;
        self.levels[level.0].orders += 1;

        if was_empty && self.is_better_than_best(price) {
            self.best = Some(level);
        }

        (level, node)
    }

    /// Fill from the head of `level`. Returns whether the head order is gone.
    pub(super) fn fill_head(&mut self, level: LevelId, quantity: f64) -> bool {
        let Some(head) = self.levels.get(level.0).and_then(|level| level.head) else {
            return false;
        };
        let remaining = self.nodes[head.0].quantity;

        if quantity + f64::EPSILON >= remaining {
            self.remove_node(level, head);
            true
        } else {
            self.nodes[head.0].quantity -= quantity;
            self.levels[level.0].quantity -= quantity;
            false
        }
    }

    /// Remove one order. Returns price, remaining quantity, and whether the
    /// level is now empty.
    pub(super) fn remove(&mut self, level: LevelId, node: NodeId) -> Option<(Price, f64, bool)> {
        let price = self.levels.get(level.0)?.price;
        let quantity = self.nodes.get(node.0)?.quantity;
        self.remove_node(level, node);
        Some((price, quantity, self.levels[level.0].orders == 0))
    }

    fn advance_best_if(&mut self, level: LevelId) {
        if self.best == Some(level) {
            self.advance_best();
        }
    }

    pub(super) fn remove_level(&mut self, level: LevelId) {
        self.advance_best_if(level);

        let (prev, next, price) = {
            let data = &self.levels[level.0];
            (data.prev, data.next, data.price)
        };

        if let Some(prev) = prev {
            self.levels[prev.0].next = next;
        } else {
            self.head = next;
        }
        if let Some(next) = next {
            self.levels[next.0].prev = prev;
        } else {
            self.tail = prev;
        }

        self.by_price.remove(&price);
        self.prices.remove(&price);
        self.free_levels.push(level);
    }

    pub(super) fn level_orders(&self, level: LevelId) -> usize {
        self.levels.get(level.0).map_or(0, |level| level.orders)
    }

    pub(super) fn order_info(&self, level: LevelId, node: NodeId) -> Option<(Price, f64)> {
        let level = self.levels.get(level.0)?;
        let node = self.nodes.get(node.0)?;
        Some((level.price, node.quantity))
    }

    pub(super) fn level_quantity(&self, price: Price) -> Option<f64> {
        self.by_price
            .get(&price)
            .map(|level| self.levels[level.0].quantity)
    }

    pub(super) fn depth(&self) -> Vec<(Price, f64, usize)> {
        let mut out = Vec::new();
        let mut current = self.best;
        while let Some(level) = current {
            let data = &self.levels[level.0];
            if data.orders > 0 {
                out.push((data.price, data.quantity, data.orders));
            }
            current = data.next;
        }
        out
    }

    fn is_better_than_best(&self, price: Price) -> bool {
        let Some(best) = self.best else {
            return true;
        };
        let best_price = self.levels[best.0].price;
        match self.side {
            Side::Buy => price > best_price,
            Side::Sell => price < best_price,
        }
    }

    fn advance_best(&mut self) {
        let mut current = self.best;
        while let Some(level) = current {
            if self.levels[level.0].orders > 0 {
                self.best = Some(level);
                return;
            }
            current = self.levels[level.0].next;
        }
        self.best = None;
    }

    fn insert_level(&mut self, price: Price) -> LevelId {
        if let Some(level) = self.by_price.get(&price) {
            return *level;
        }

        // Find the level immediately better than `price`; the linked list is
        // best-to-worst. Bids descend, asks ascend.
        let predecessor = match self.side {
            Side::Buy => self.prices.range(price..).next().copied(),
            Side::Sell => self.prices.range(..price).next_back().copied(),
        };
        let (prev, next) = match predecessor {
            Some(prev_price) => {
                let prev_level = *self
                    .by_price
                    .get(&prev_price)
                    .expect("price index and level map out of sync");
                (Some(prev_level), self.levels[prev_level.0].next)
            }
            None => (None, self.head),
        };

        let level = self.alloc_level(price);
        self.levels[level.0].prev = prev;
        self.levels[level.0].next = next;

        if let Some(prev) = prev {
            self.levels[prev.0].next = Some(level);
        } else {
            self.head = Some(level);
        }
        if let Some(next) = next {
            self.levels[next.0].prev = Some(level);
        } else {
            self.tail = Some(level);
        }

        self.by_price.insert(price, level);
        self.prices.insert(price);
        level
    }

    fn remove_node(&mut self, level: LevelId, node: NodeId) {
        let (prev, next, quantity) = {
            let data = &self.nodes[node.0];
            (data.prev, data.next, data.quantity)
        };

        if let Some(prev) = prev {
            self.nodes[prev.0].next = next;
        } else {
            self.levels[level.0].head = next;
        }
        if let Some(next) = next {
            self.nodes[next.0].prev = prev;
        } else {
            self.levels[level.0].tail = prev;
        }

        self.levels[level.0].quantity -= quantity;
        self.levels[level.0].orders -= 1;
        self.free_nodes.push(node);
    }

    fn alloc_level(&mut self, price: Price) -> LevelId {
        match self.free_levels.pop() {
            Some(level) => {
                self.levels[level.0] = Level::new(price);
                level
            }
            None => {
                let level = LevelId(self.levels.len());
                self.levels.push(Level::new(price));
                level
            }
        }
    }

    fn alloc_node(&mut self, id: OrderId, quantity: f64) -> NodeId {
        match self.free_nodes.pop() {
            Some(node) => {
                self.nodes[node.0] = Node::new(id, quantity);
                node
            }
            None => {
                let node = NodeId(self.nodes.len());
                self.nodes.push(Node::new(id, quantity));
                node
            }
        }
    }
}
