use std::io::{Read, Write};

use polars::io::json::{JsonFormat, JsonReader, JsonWriter};
use polars::io::{SerReader, SerWriter};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::serialization::{serializable, Error};
use crate::DataFrame;

impl DataFrame {
    pub fn serialize_json(&self, writer: &mut dyn Write) -> Result<(), Error> {
        let mut df = self.clone().into_inner();
        JsonWriter::new(writer)
            .with_json_format(JsonFormat::Json)
            .finish(&mut df)
            .map_err(Error::Polars)
    }

    pub fn deserialize_json(reader: &mut dyn Read) -> Result<Self, Error> {
        let mut buf = Vec::new();
        reader.read_to_end(&mut buf)?;
        let df = JsonReader::new(std::io::Cursor::new(buf))
            .with_json_format(JsonFormat::Json)
            .finish()
            .map_err(Error::Polars)?;
        Ok(DataFrame::new(df))
    }
}

impl Serialize for DataFrame {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut buf = Vec::new();
        self.serialize_json(&mut buf)
            .map_err(|e| serde::ser::Error::custom(e.to_string()))?;
        let value: serde_json::Value =
            serde_json::from_slice(&buf).map_err(|e| serde::ser::Error::custom(e.to_string()))?;
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DataFrame {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let buf =
            serde_json::to_vec(&value).map_err(|e| serde::de::Error::custom(e.to_string()))?;
        let mut reader = buf.as_slice();
        DataFrame::deserialize_json(&mut reader)
            .map_err(|e| serde::de::Error::custom(e.to_string()))
    }
}

serializable!(DataFrame, [Json]);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialization::{Protocol, Serializable};

    #[test]
    fn dataframe_json_roundtrip() {
        use polars::prelude::Column;

        let inner = polars::prelude::DataFrame::new(vec![
            Column::new("symbol".into(), &["AAPL", "MSFT"]),
            Column::new("close".into(), &[150.0f64, 415.0]),
        ])
        .unwrap();
        let df = DataFrame::new(inner);

        let mut buf = Vec::new();
        Serializable::serialize(&df, Protocol::Json, &mut buf).unwrap();
        let back =
            <DataFrame as Serializable>::deserialize(Protocol::Json, &mut buf.as_slice()).unwrap();

        assert_eq!(back.height(), 2);
        assert_eq!(
            back.column("symbol").unwrap().str().unwrap().get(0),
            Some("AAPL")
        );
        assert_eq!(
            back.column("close").unwrap().f64().unwrap().get(1),
            Some(415.0)
        );
    }

    #[test]
    fn dataframe_serde_roundtrip() {
        use polars::prelude::Column;

        let inner = polars::prelude::DataFrame::new(vec![
            Column::new("symbol".into(), &["AAPL", "MSFT"]),
            Column::new("close".into(), &[150.0f64, 415.0]),
        ])
        .unwrap();
        let df = DataFrame::new(inner);

        let json = serde_json::to_value(&df).unwrap();
        let back: DataFrame = serde_json::from_value(json).unwrap();

        assert_eq!(back.height(), 2);
        assert_eq!(
            back.column("symbol").unwrap().str().unwrap().get(0),
            Some("AAPL")
        );
    }
}
