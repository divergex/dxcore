use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::Instrument;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstrumentStore {
    by_id: HashMap<i32, Instrument>,
    by_symbol: HashMap<String, Instrument>,
}

impl InstrumentStore {
    pub fn insert(&mut self, instrument: Instrument) {
        self.by_id
            .insert(instrument.contract_id, instrument.clone());
        self.by_symbol
            .insert(instrument.symbol.clone(), instrument);
    }

    pub fn get(&self, contract_id: i32) -> Option<&Instrument> {
        self.by_id.get(&contract_id)
    }

    pub fn get_by_symbol(&self, symbol: &str) -> Option<&Instrument> {
        self.by_symbol.get(symbol)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}
