use polars::prelude::*;

use super::View;

pub struct TickView {
    pub timestamp_col: String,
    pub col_map: Vec<(String, String)>,
}

impl TickView {
    pub fn new(timestamp_col: impl Into<String>) -> Self {
        Self {
            timestamp_col: timestamp_col.into(),
            col_map: Vec::new(),
        }
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
            history
                .vstack_mut(step)
                .expect("failed to append tick step to history");
        }
    }

    fn frame<'a>(&self, step: &'a DataFrame) -> &'a DataFrame {
        step
    }

    fn step_timestamp(&self, step: &DataFrame) -> Option<i64> {
        let column = self
            .col_map
            .iter()
            .find(|(from, _)| from == &self.timestamp_col)
            .map(|(_, to)| to.as_str())
            .unwrap_or(&self.timestamp_col);
        timestamp_at(step, column)
    }
}

fn timestamp_at(df: &DataFrame, column: &str) -> Option<i64> {
    let series = df.column(column).ok()?.as_materialized_series();
    match series.dtype() {
        DataType::Date => series.date().ok()?.get(0).map(i64::from),
        DataType::Datetime(_, _) => series.datetime().ok()?.get(0),
        DataType::Int8 => series.i8().ok()?.get(0).map(i64::from),
        DataType::Int16 => series.i16().ok()?.get(0).map(i64::from),
        DataType::Int32 => series.i32().ok()?.get(0).map(i64::from),
        DataType::Int64 => series.i64().ok()?.get(0),
        DataType::UInt8 => series.u8().ok()?.get(0).map(i64::from),
        DataType::UInt16 => series.u16().ok()?.get(0).map(i64::from),
        DataType::UInt32 => series.u32().ok()?.get(0).map(i64::from),
        DataType::UInt64 => series
            .u64()
            .ok()?
            .get(0)
            .and_then(|value| i64::try_from(value).ok()),
        _ => None,
    }
}
