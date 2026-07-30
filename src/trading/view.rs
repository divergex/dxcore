use polars::prelude::*;

/// A View defines how a DataFrame is sliced into steps for the execution loop. 
/// It is responsible for translating source-column names into the strategy's expected column names.
pub trait View {
    type Item;

    /// Each step has had `col_map` applied, so the strategy sees only its canonical columns.
    fn steps(&self, df: &DataFrame) -> impl Iterator<Item = Self::Item>;

    fn append(&self, history: &mut DataFrame, step: &Self::Item);
}

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
}

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
}
