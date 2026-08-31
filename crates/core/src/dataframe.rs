use std::ops::{Deref, DerefMut};

use polars::prelude::DataFrame as PolarsDataFrame;

/// Derefs to the polars frame, so all polars column/row APIs remain available.
#[derive(Debug, Clone, Default)]
pub struct DataFrame {
    inner: PolarsDataFrame,
}

impl DataFrame {
    pub fn new(inner: PolarsDataFrame) -> Self {
        Self { inner }
    }

    pub fn into_inner(self) -> PolarsDataFrame {
        self.inner
    }
}

impl From<PolarsDataFrame> for DataFrame {
    fn from(inner: PolarsDataFrame) -> Self {
        Self::new(inner)
    }
}

impl From<DataFrame> for PolarsDataFrame {
    fn from(df: DataFrame) -> Self {
        df.into_inner()
    }
}

impl Deref for DataFrame {
    type Target = PolarsDataFrame;

    fn deref(&self) -> &PolarsDataFrame {
        &self.inner
    }
}

impl DerefMut for DataFrame {
    fn deref_mut(&mut self) -> &mut PolarsDataFrame {
        &mut self.inner
    }
}
