use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use serde_json::Value;

use ::dxcore::trading::{
    Order, OrderData, OrderPrice, OrderQuantity, OrderType, Side, Signal,
};

/// Direction of an order.
#[pyclass(name = "Side", module = "dxcore", eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PySide {
    Buy,
    Sell,
}

impl PySide {
    fn from_core(side: Side) -> Self {
        match side {
            Side::Buy => Self::Buy,
            Side::Sell => Self::Sell,
        }
    }
}

/// How an order is specified.
#[pyclass(name = "OrderType", module = "dxcore", eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyOrderType {
    Market,
    Limit,
    StopLoss,
}

impl PyOrderType {
    fn from_core(order_type: OrderType) -> Self {
        match order_type {
            OrderType::Market => Self::Market,
            OrderType::Limit => Self::Limit,
            OrderType::StopLoss => Self::StopLoss,
        }
    }

    fn to_core(self) -> OrderType {
        match self {
            Self::Market => OrderType::Market,
            Self::Limit => OrderType::Limit,
            Self::StopLoss => OrderType::StopLoss,
        }
    }
}

/// Size of an order: a fraction of the account (`Pct`) or a share count (`Abs`).
#[pyclass(name = "OrderQuantity", module = "dxcore", eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PyOrderQuantity {
    Pct(f64),
    Abs(f64),
}

impl PyOrderQuantity {
    fn from_core(quantity: OrderQuantity) -> Self {
        match quantity {
            OrderQuantity::Pct(value) => Self::Pct(value),
            OrderQuantity::Abs(value) => Self::Abs(value),
        }
    }

    fn to_core(self) -> OrderQuantity {
        match self {
            Self::Pct(value) => OrderQuantity::Pct(value),
            Self::Abs(value) => OrderQuantity::Abs(value),
        }
    }
}

/// Price an order is specified at: at the market (`Mkt`) or a fixed price (`Px`).
///
/// [`Mkt`](PyOrderPrice::Mkt) is exposed as a class attribute holding the
/// market-price value, so usage reads `OrderPrice.Mkt`; a fixed price is built
/// with `OrderPrice.Px(101.5)`.
#[pyclass(name = "OrderPrice", module = "dxcore", eq, from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyOrderPrice {
    inner: OrderPrice,
}

#[pymethods]
impl PyOrderPrice {
    /// Execute at whatever the market gives.
    #[classattr]
    #[allow(non_snake_case)]
    fn Mkt() -> PyOrderPrice {
        PyOrderPrice {
            inner: OrderPrice::Mkt,
        }
    }

    /// Execute at a fixed price (a limit price, or a stop level).
    #[staticmethod]
    #[allow(non_snake_case)]
    fn Px(price: f64) -> PyOrderPrice {
        PyOrderPrice {
            inner: OrderPrice::Px(price),
        }
    }

    fn __repr__(&self) -> String {
        match self.inner {
            OrderPrice::Mkt => "OrderPrice.Mkt".to_string(),
            OrderPrice::Px(price) => format!("OrderPrice.Px({price})"),
        }
    }
}

impl PyOrderPrice {
    fn from_core(inner: OrderPrice) -> Self {
        Self { inner }
    }

    fn to_core(&self) -> OrderPrice {
        self.inner
    }
}

/// The position a strategy wants to hold in one instrument after a step.
#[pyclass(name = "Signal", module = "dxcore", from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct PySignal {
    inner: Signal,
}

#[pymethods]
impl PySignal {
    #[new]
    #[pyo3(signature = (shares, symbol = None))]
    fn new(shares: f64, symbol: Option<String>) -> Self {
        Self {
            inner: Signal { symbol, shares },
        }
    }

    #[getter]
    fn symbol(&self) -> Option<String> {
        self.inner.symbol.clone()
    }

    #[getter]
    fn shares(&self) -> f64 {
        self.inner.shares
    }

    fn __repr__(&self) -> String {
        format!(
            "Signal(shares={}, symbol={:?})",
            self.inner.shares, self.inner.symbol
        )
    }
}

impl PySignal {
    pub(crate) fn to_core(&self) -> Signal {
        self.inner.clone()
    }
}

/// An order a strategy authored directly.
#[pyclass(name = "OrderData", module = "dxcore", from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyOrderData {
    inner: OrderData,
}

#[pymethods]
impl PyOrderData {
    #[new]
    #[pyo3(signature = (quantity, order_type, price, symbol = None))]
    fn new(
        quantity: PyOrderQuantity,
        order_type: PyOrderType,
        price: PyOrderPrice,
        symbol: Option<String>,
    ) -> Self {
        Self {
            inner: OrderData {
                symbol,
                quantity: quantity.to_core(),
                order_type: order_type.to_core(),
                price: price.to_core(),
            },
        }
    }

    #[getter]
    fn symbol(&self) -> Option<String> {
        self.inner.symbol.clone()
    }

    #[getter]
    fn quantity(&self) -> PyOrderQuantity {
        PyOrderQuantity::from_core(self.inner.quantity)
    }

    #[getter]
    fn order_type(&self) -> PyOrderType {
        PyOrderType::from_core(self.inner.order_type)
    }

    #[getter]
    fn price(&self) -> PyOrderPrice {
        PyOrderPrice::from_core(self.inner.price)
    }

    fn __repr__(&self) -> String {
        let data = &self.inner;
        format!(
            "OrderData(quantity={:?}, order_type={:?}, price={:?}, symbol={:?})",
            data.quantity, data.order_type, data.price, data.symbol
        )
    }
}

impl PyOrderData {
    pub(crate) fn to_core(&self) -> OrderData {
        self.inner.clone()
    }
}

/// A validated order produced by an engine.
#[pyclass(name = "Order", module = "dxcore", from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyOrder {
    inner: Order,
}

#[pymethods]
impl PyOrder {
    #[getter]
    fn date(&self) -> Option<i64> {
        self.inner.date
    }

    #[getter]
    fn symbol(&self) -> &str {
        &self.inner.symbol
    }

    #[getter]
    fn side(&self) -> PySide {
        PySide::from_core(self.inner.side)
    }

    #[getter]
    fn quantity(&self) -> PyOrderQuantity {
        PyOrderQuantity::from_core(self.inner.quantity)
    }

    #[getter]
    fn order_type(&self) -> PyOrderType {
        PyOrderType::from_core(self.inner.order_type)
    }

    #[getter]
    fn price(&self) -> PyOrderPrice {
        PyOrderPrice::from_core(self.inner.price)
    }

    fn __repr__(&self) -> String {
        let order = &self.inner;
        let date = match order.date {
            Some(date) => date.to_string(),
            None => "None".to_string(),
        };
        let side = match order.side {
            Side::Buy => "Buy",
            Side::Sell => "Sell",
        };
        let quantity = match order.quantity {
            OrderQuantity::Pct(value) => format!("Pct({value})"),
            OrderQuantity::Abs(value) => format!("Abs({value})"),
        };
        let order_type = match order.order_type {
            OrderType::Market => "Market",
            OrderType::Limit => "Limit",
            OrderType::StopLoss => "StopLoss",
        };
        let price = match order.price {
            OrderPrice::Mkt => "Mkt".to_string(),
            OrderPrice::Px(value) => format!("Px({value})"),
        };
        format!(
            "Order(date={date}, symbol={:?}, side={side}, quantity={quantity}, \
             order_type={order_type}, price={price})",
            order.symbol
        )
    }
}

impl PyOrder {
    pub(crate) fn from_core(inner: Order) -> Self {
        Self { inner }
    }
}

pub(crate) fn output_from_json(py: Python<'_>, value: Value) -> PyResult<Py<PyAny>> {
    match value {
        Value::Null => Ok(py.None()),
        other => {
            if let Ok(signal) = serde_json::from_value::<Signal>(other.clone()) {
                return Ok(Py::new(py, PySignal { inner: signal })?.into_any());
            }
            if let Ok(data) = serde_json::from_value::<OrderData>(other.clone()) {
                return Ok(Py::new(py, PyOrderData { inner: data })?.into_any());
            }
            Err(PyTypeError::new_err(format!(
                "strategy output must be None, Signal, or OrderData, got {other}"
            )))
        }
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PySide>()?;
    m.add_class::<PyOrderType>()?;
    m.add_class::<PyOrderQuantity>()?;
    m.add_class::<PyOrderPrice>()?;
    m.add_class::<PySignal>()?;
    m.add_class::<PyOrderData>()?;
    m.add_class::<PyOrder>()?;
    Ok(())
}
