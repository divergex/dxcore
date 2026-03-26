# Design

This project implements a matching engine in C++.

The main goal is to simulate a matching engine for financial orders.
It should be able to process limit and market orders, 
and correctly match them accordingly.

The insertion, deletion and matching should be performed with 
optimal asymptotic efficiency.
The insertion of limit orders should follow time-priority, and insert/cancel
should be O(1).


## Details

The order book structure should store price levels for bids and asks.
Each price level stores orders by the time of insertion, 
as well as an order id map to cancel orders in O(1).

