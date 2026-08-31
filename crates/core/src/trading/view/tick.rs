use polars::prelude::*;

use super::View;

pub struct TickView {
    pub timestamp_col: String,
    pub col_map: Vec<(String, String)>,
}

impl TickView {
    pub fn new(timestamp_col: impl Into<String>) -> Self {
        Self { timestamp_col: timestamp_col.into(), col_map: Vec::new() }
    }

    pub fn with_col_map(mut self, map: Vec<(String, String)>) -> Self {
        self.col_map = map;
        self
    }
}

impl View for TickView {
    type Item = DataFrame;

    fn steps(&self, df: &DataFrame) -> impl Iterator<Item = DataFrame> {
        let df = df.clone();
        let n = df.height();
        let col_map = self.col_map.clone();
        (0..n).map(move |i| {
            let mut row = df.slice(i as i64, 1);
            for (from, to) in &col_map {
                row.rename(from, to.into()).unwrap();
            }
            row
        })
    }

    fn append(&self, history: &mut DataFrame, step: &DataFrame) {
        if history.width() == 0 {
            *history = step.clone();
        } else {
            history.vstack_mut(step).expect("failed to append tick step to history");
        }
    }
}
