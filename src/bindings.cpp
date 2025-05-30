//
// Created by rzimmerdev on 10/15/24.
//

#ifndef BINDINGS_H
#define BINDINGS_H

#include <pybind11/pybind11.h>
#include "core/orders/order_book.h"

namespace py = pybind11;

PYBIND11_MODULE(dxcore, m) {
    py::class_<dxcore::Order>(m, "Order")
        .def(py::init<int, float>()) // default constructor
        .def_readwrite("price", &dxcore::Order::price)
        .def_readwrite("quantity", &dxcore::Order::quantity);
        // expose other fields as needed
    //
    // py::class_<dxcore::OrderBook>(m, "OrderBook")
    //     .def(py::init<>())
    //     .def("insert_ask", &dxcore::OrderBook::insert_ask)
    //     .def("insert_bid", &dxcore::OrderBook::insert_bid);
}



#endif //BINDINGS_H
