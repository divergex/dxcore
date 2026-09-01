use pyo3::prelude::*;

use ::dxcore::trading::DailyView;

#[pyclass(name = "DailyView", module = "dxcore")]
pub struct PyDailyView {
    date_col: String,
    col_map: Vec<(String, String)>,
}

impl PyDailyView {
    pub(crate) fn to_core(&self) -> DailyView {
        DailyView {
            date_col: self.date_col.clone(),
            col_map: self.col_map.clone(),
        }
    }
}

#[pymethods]
impl PyDailyView {
    #[new]
    #[pyo3(signature = (date_col, col_map=None))]
    fn new(date_col: String, col_map: Option<Vec<(String, String)>>) -> Self {
        Self {
            date_col,
            col_map: col_map.unwrap_or_default(),
        }
    }

    #[getter]
    fn col_map(&self) -> Vec<(String, String)> {
        self.col_map.clone()
    }
}

pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyDailyView>()?;
    Ok(())
}
