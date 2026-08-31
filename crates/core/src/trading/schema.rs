use std::ops::Deref;
use polars::prelude::*;

pub struct KeyedSchema {
    pub keys: Vec<String>,
    schema: Schema,
}

impl KeyedSchema {
    pub fn new(keys: Vec<String>, schema: Schema) -> Self {
        Self { keys, schema }
    }

    pub fn value_cols(&self) -> Vec<String> {
        self.schema
            .iter_fields()
            .map(|f| f.name().to_string())
            .filter(|name| !self.keys.contains(name))
            .collect()
    }
}

impl Deref for KeyedSchema {
    type Target = Schema;

    fn deref(&self) -> &Schema {
        &self.schema
    }
}
