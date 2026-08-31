use crate::core::{AccountMetric, Instrument, InstrumentStore, Portfolio};
use crate::serialization::{Registry, serde_serializable};

serde_serializable!(Instrument, [Json]);
serde_serializable!(AccountMetric, [Json]);
serde_serializable!(Portfolio, [Json]);
serde_serializable!(InstrumentStore, [Json]);

/// Register all core types (plus [`crate::DataFrame`]) into `registry`.
pub fn register_defaults(registry: &mut Registry) {
    registry.register::<Instrument>().expect("core types register once");
    registry.register::<AccountMetric>().expect("core types register once");
    registry.register::<Portfolio>().expect("core types register once");
    registry.register::<InstrumentStore>().expect("core types register once");
    registry.register::<crate::DataFrame>().expect("core types register once");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataFrame;
    use crate::serialization::{Protocol, Serializable};

    fn aapl() -> Instrument {
        Instrument {
            contract_id: 265598,
            symbol: "AAPL".into(),
            security_type: "STK".into(),
            exchange: "SMART".into(),
            currency: "USD".into(),
        }
    }

    #[test]
    fn instrument_json_roundtrip() {
        let inst = aapl();
        let mut buf = Vec::new();
        inst.serialize(Protocol::Json, &mut buf).unwrap();
        let back = Instrument::deserialize(Protocol::Json, &mut buf.as_slice()).unwrap();
        assert_eq!(back, inst);
    }

    #[test]
    fn portfolio_json_roundtrip_nests_children() {
        let mut p = Portfolio::default();
        p.upsert_metric("NetLiquidation".into(), "100000".into(), "USD".into());
        p.set_holding(aapl(), 100.0);

        let mut buf = Vec::new();
        p.serialize(Protocol::Json, &mut buf).unwrap();
        let back = Portfolio::deserialize(Protocol::Json, &mut buf.as_slice()).unwrap();

        assert_eq!(back.metrics["NetLiquidation"].value, "100000");
        assert_eq!(back.quantity(265598), Some(100.0));
        assert_eq!(back.instrument(265598).unwrap().symbol, "AAPL");
    }

    #[test]
    fn protocols_advertise_json() {
        assert_eq!(Instrument::protocols(), &[Protocol::Json]);
        assert_eq!(DataFrame::protocols(), &[Protocol::Json]);
    }
}
