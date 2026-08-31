use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Instrument {
    pub contract_id: i32,
    pub symbol: String,
    pub security_type: String,
    pub exchange: String,
    pub currency: String,
}

impl std::fmt::Display for Instrument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.symbol, self.security_type)
    }
}
