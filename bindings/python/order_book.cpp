#include <pybind11/pybind11.h>
#include <pybind11/stl.h>  // std::vector<Transaction> → list automatically

#include <boost/uuid/uuid.hpp>
#include <boost/uuid/uuid_io.hpp>

#include "dxcore.h"

namespace py = pybind11;

PYBIND11_MODULE(order_book, m) {
    m.doc() = "Order book matching engine";

    py::class_<Transaction>(m, "Transaction", "Represents a matched trade")
            .def_readonly("price",    &Transaction::price)
            .def_readonly("quantity", &Transaction::quantity)
            // Use a lambda to convert the UUID to a string for Python
            .def_property_readonly("bid_id", [](const Transaction &t) {
                return boost::uuids::to_string(t.buyOrderId);
            })
            .def_property_readonly("ask_id", [](const Transaction &t) {
                return boost::uuids::to_string(t.sellOrderId);
            });

    py::class_<Instrument>(m, "Instrument", "A traded instrument")
        .def(py::init<std::string>(), py::arg("symbol"), "Create instrument with symbol")
        .def_readonly("symbol",   &Instrument::symbol,   "Instrument symbol")
        .def_readonly("exchange", &Instrument::exchange, "Exchange name");

    py::class_<Order>(m, "Order", "A single order")
        .def(py::init<OrderType, Side, double, uint64_t>(),
             py::arg("type"), py::arg("side"), py::arg("price"), py::arg("quantity"),
             "Create a new order")
        .def_property_readonly("id", [](const Order &o) {
            return boost::uuids::to_string(o.getId());
        }, "Order identifier")
        .def_readonly("price",    &Order::price,    "Order price")
        .def_readonly("quantity", &Order::quantity, "Order quantity")
        .def_readonly("side",     &Order::side,     "Order side (BID or ASK)");

    py::enum_<Side>(m, "Side", "Buy or sell side of an order")
        .value("Buy", Side::Buy)
        .value("Sell", Side::Sell)
        .export_values();

    py::enum_<OrderType>(m, "OrderType", "Buy or sell side of an order")
        .value("Market", OrderType::Market)
        .value("Limit", OrderType::Limit)
        .export_values();

    py::class_<OrderBook>(m, "OrderBook", "Order book for a given instrument")
        .def(py::init<Instrument, double>(),
             py::arg("instrument"), py::arg("tick_size") = 0.01,
             "Create order book for an instrument with optional tick size")
        .def("insert", &OrderBook::insert, "Insert a new order, returns list of Transactions")
        .def("cancel", &OrderBook::cancel, "Cancel an order by ID")
        .def("get_mid_price", &OrderBook::getMidPrice, "Return the current mid price")
        .def("get_best_bid",  &OrderBook::getBestBid,  "Return the best bid")
        .def("get_best_ask",  &OrderBook::getBestAsk,  "Return the best ask")
        .def("get_level_count", &OrderBook::getLevelCount, "Return number of price levels")
        .def("get_instrument", &OrderBook::getInstrument,
             py::return_value_policy::reference_internal,
             "Return the associated Instrument");
}