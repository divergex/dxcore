use polars::prelude::*;

use super::View;

pub struct PanelStep {
    pub date: i32,
    pub symbol: String,
    pub data: DataFrame,
}

/// Yields one PanelStep per unique (date, symbol) pair.
pub struct PanelView {
    pub date_col: String,
    pub symbol_col: String,
    pub col_map: Vec<(String, String)>,
}

impl PanelView {
    pub fn new(date_col: impl Into<String>, symbol_col: impl Into<String>) -> Self {
        Self {
            date_col: date_col.into(),
            symbol_col: symbol_col.into(),
            col_map: Vec::new(),
        }
    }

    pub fn with_col_map(mut self, map: Vec<(String, String)>) -> Self {
        self.col_map = map;
        self
    }
}

impl View for PanelView {
    type Item = PanelStep;

    fn steps(&self, df: &DataFrame) -> impl Iterator<Item = PanelStep> {
        let df = df.clone();
        let date_col = self.date_col.clone();
        let symbol_col = self.symbol_col.clone();
        let col_map = self.col_map.clone();

        let pairs: Vec<(i32, String)> = {
            let dates = df.column(&date_col).unwrap().date().unwrap().clone();
            let symbols = df.column(&symbol_col).unwrap().str().unwrap().clone();
            let mut pairs: Vec<(i32, String)> = dates
                .into_iter()
                .zip(symbols.into_iter())
                .filter_map(|(d, s)| Some((d?, s?.to_string())))
                .collect();
            pairs.sort_unstable();
            pairs.dedup();
            pairs
        };

        pairs.into_iter().map(move |(date, symbol)| {
            let date_series = df.column(&date_col).unwrap();
            let symbol_series = df.column(&symbol_col).unwrap();
            let date_mask = date_series.date().unwrap().equal(date);
            let symbol_strs = symbol_series.str().unwrap();
            let symbol_values: Vec<bool> = symbol_strs
                .into_iter()
                .map(|s| s == Some(symbol.as_ref()))
                .collect();
            let symbol_mask = BooleanChunked::from_slice("".into(), &symbol_values);
            let mask = date_mask & symbol_mask;
            let mut data = df.filter(&mask).unwrap();
            for (from, to) in &col_map {
                data.rename(from, to.into()).unwrap();
            }
            PanelStep { date, symbol, data }
        })
    }

    fn append(&self, history: &mut DataFrame, step: &PanelStep) {
        if history.width() == 0 {
            *history = step.data.clone();
        } else {
            history
                .vstack_mut(&step.data)
                .expect("failed to append panel step to history");
        }
    }

    fn step_ord_key(&self, step: &PanelStep) -> Option<i64> {
        Some(step.date as i64)
    }
}
