use std::io::{Read, Write};

use polars::io::json::{JsonFormat, JsonReader, JsonWriter};
use polars::io::{SerReader, SerWriter};

use crate::DataFrame;
use crate::serialization::{Error, serializable};

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
        df.serialize(Protocol::Json, &mut buf).unwrap();
        let back = DataFrame::deserialize(Protocol::Json, &mut buf.as_slice()).unwrap();

        assert_eq!(back.height(), 2);
        assert_eq!(back.column("symbol").unwrap().str().unwrap().get(0), Some("AAPL"));
        assert_eq!(back.column("close").unwrap().f64().unwrap().get(1), Some(415.0));
    }
}
