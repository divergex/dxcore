use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use super::Error;

/// Serialize any serde-serializable value to the JSON wire format.
pub fn to_value<T: Serialize>(value: &T) -> Result<Value, Error> {
    serde_json::to_value(value).map_err(Error::Json)
}

/// Deserialize the JSON wire format back into a typed value.
pub fn from_value<T: DeserializeOwned>(value: Value) -> Result<T, Error> {
    serde_json::from_value(value).map_err(Error::Json)
}
