use polars::prelude::*;

pub fn ohlc_df() -> DataFrame {
    let date_col = Column::new(
        "date".into(),
        Series::new("date".into(), &[19000i32, 19000, 19001, 19001, 19002])
            .cast(&DataType::Date)
            .unwrap(),
    );
    let symbol_col = Column::new("symbol".into(), &["AAPL", "GOOG", "AAPL", "GOOG", "AAPL"]);
    let open_col = Column::new("open".into(), &[150.0f64, 140.0, 151.0, 141.0, 152.0]);
    let close_col = Column::new("close".into(), &[155.0f64, 145.0, 156.0, 146.0, 153.0]);

    DataFrame::new(vec![date_col, symbol_col, open_col, close_col]).unwrap()
}

pub fn empty_ohlc_df() -> DataFrame {
    DataFrame::new(vec![
        Column::new_empty("date".into(), &DataType::Date),
        Column::new_empty("symbol".into(), &DataType::String),
        Column::new_empty("open".into(), &DataType::Float64),
        Column::new_empty("close".into(), &DataType::Float64),
    ])
    .unwrap()
}
