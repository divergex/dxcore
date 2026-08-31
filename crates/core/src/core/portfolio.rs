use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{AccountMetric, Instrument};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Portfolio {
    pub metrics: HashMap<String, AccountMetric>,
    holdings: HashMap<i32, (Instrument, f64)>,
}

impl Portfolio {
    pub fn upsert_metric(&mut self, key: String, value: String, currency: String) {
        self.metrics.insert(
            key.clone(),
            AccountMetric {
                key,
                value,
                currency,
            },
        );
    }

    pub fn set_holding(&mut self, instrument: Instrument, quantity: f64) {
        if quantity == 0.0 {
            self.holdings.remove(&instrument.contract_id);
        } else {
            self.holdings
                .insert(instrument.contract_id, (instrument, quantity));
        }
    }

    pub fn instrument(&self, contract_id: i32) -> Option<&Instrument> {
        self.holdings.get(&contract_id).map(|(inst, _)| inst)
    }

    pub fn quantity(&self, contract_id: i32) -> Option<f64> {
        self.holdings.get(&contract_id).map(|(_, qty)| *qty)
    }

    pub fn holding_count(&self) -> usize {
        self.holdings.len()
    }

    pub fn holdings(&self) -> impl Iterator<Item = (&Instrument, f64)> {
        self.holdings.values().map(|(inst, qty)| (inst, *qty))
    }
}
