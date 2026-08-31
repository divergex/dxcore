use polars::prelude::*;
use dxlib::trading::KeyedSchema;

#[test]
fn value_cols_returns_non_key_columns() {
    let ks = KeyedSchema::new(
        vec!["date".into(), "symbol".into()],
        Schema::from_iter([
            Field::new("date".into(), DataType::Date),
            Field::new("symbol".into(), DataType::String),
            Field::new("open".into(), DataType::Float64),
            Field::new("close".into(), DataType::Float64),
        ]),
    );

    let mut vals = ks.value_cols();
    vals.sort();
    assert_eq!(vals, vec!["close", "open"]);
}

#[test]
fn value_cols_empty_when_all_are_keys() {
    let ks = KeyedSchema::new(
        vec!["a".into(), "b".into()],
        Schema::from_iter([
            Field::new("a".into(), DataType::Int32),
            Field::new("b".into(), DataType::String),
        ]),
    );

    assert!(ks.value_cols().is_empty());
}

#[test]
fn deref_accesses_schema_methods() {
    let ks = KeyedSchema::new(
        vec!["date".into()],
        Schema::from_iter([Field::new("date".into(), DataType::Date)]),
    );

    assert_eq!(ks.get("date"), Some(&DataType::Date));
    assert_eq!(ks.get("nonexistent"), None);
}
