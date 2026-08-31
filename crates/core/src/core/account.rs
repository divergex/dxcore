use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountMetric {
    pub key: String,
    pub value: String,
    pub currency: String,
}
