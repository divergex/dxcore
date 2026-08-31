use polars::prelude::*;

use super::View;

pub struct DailyView {
    pub date_col: String,
    pub col_map: Vec<(String, String)>,
}

impl DailyView {
    pub fn new(date_col: impl Into<String>) -> Self {
        Self { date_col: date_col.into(), col_map: Vec::new() }
    }

    pub fn with_col_map(mut self, map: Vec<(String, String)>) -> Self {
        self.col_map = map;
        self
    }
}

impl View for DailyView {
    type Item = (i32, DataFrame);

    fn steps(&self, df: &DataFrame) -> impl Iterator<Item = (i32, DataFrame)> {
        let date_series = df.column(&self.date_col).unwrap().clone();
        let dates: Vec<i32> = date_series
            .date()
            .unwrap()
            .into_iter()
            .flatten()
            .collect();

        let mut unique_dates = dates.clone();
        unique_dates.sort_unstable();
        unique_dates.dedup();

        let df = df.clone();
        let col_map = self.col_map.clone();

        unique_dates.into_iter().map(move |date| {
            let mask = date_series.date().unwrap().equal(date);
            let mut chunk = df.filter(&mask).unwrap();
            for (from, to) in &col_map {
                chunk.rename(from, to.into()).unwrap();
            }
            (date, chunk)
        })
    }

    fn append(&self, history: &mut DataFrame, step: &(i32, DataFrame)) {
        if history.width() == 0 {
            *history = step.1.clone();
        } else {
            history
                .vstack_mut(&step.1)
                .expect("failed to append daily step to history");
        }
    }

    fn step_ord_key(&self, step: &(i32, DataFrame)) -> Option<i64> {
        Some(step.0 as i64)
    }
}
