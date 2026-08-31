use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Mutex, MutexGuard, OnceLock};

use serde_json::Value;

use super::{Error, Protocol, Serializable, register_defaults};

/// A registry of type names to (de)serializers, enabling tagged payloads:
/// `{"type": "Instrument", "data": …}`.
#[derive(Default)]
pub struct Registry {
    by_name: HashMap<&'static str, Entry>,
    by_type: HashMap<TypeId, &'static str>,
}

struct Entry {
    protocols: &'static [Protocol],
    serialize: fn(&dyn Any, Protocol, &mut dyn Write) -> Result<(), Error>,
    deserialize: fn(Protocol, &mut dyn Read) -> Result<Box<dyn Any>, Error>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<T: Serializable + Any>(&mut self) -> Result<(), Error> {
        let name = std::any::type_name::<T>();
        if self.by_type.contains_key(&TypeId::of::<T>()) {
            return Ok(());
        }

        self.by_type.insert(TypeId::of::<T>(), name);
        self.by_name.insert(
            name,
            Entry {
                protocols: T::protocols(),
                serialize: |value, protocol, writer| {
                    let ty = value
                        .downcast_ref::<T>()
                        .expect("registry entry keyed by TypeId, downcast cannot fail");
                    ty.serialize(protocol, writer)
                },
                deserialize: |protocol, reader| {
                    T::deserialize(protocol, reader).map(|v| Box::new(v) as Box<dyn Any>)
                },
            },
        );
        Ok(())
    }

    pub fn is_registered(&self, name: &str) -> bool {
        self.by_name.contains_key(name)
    }

    pub fn protocols(&self, name: &str) -> Option<&'static [Protocol]> {
        self.by_name.get(name).map(|e| e.protocols)
    }

    /// The concrete type must be registered.
    pub fn serialize_erased(
        &self,
        value: &dyn Any,
        protocol: Protocol,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        let name = self
            .by_type
            .get(&value.type_id())
            .ok_or_else(|| Error::UnknownType(format!("{:?}", value.type_id())))?;
        let entry = self.by_name.get(name).expect("by_type and by_name stay in sync");
        (entry.serialize)(value, protocol, writer)
    }

    pub fn deserialize_tagged(&self, payload: &[u8]) -> Result<Box<dyn Any>, Error> {
        let value: Value = serde_json::from_slice(payload)?;
        let type_name = value
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::UnknownType("missing 'type' field".into()))?;
        let entry = self
            .by_name
            .get(type_name)
            .ok_or_else(|| Error::UnknownType(type_name.to_string()))?;
        let data = value
            .get("data")
            .ok_or_else(|| Error::UnknownType("missing 'data' field".into()))?;
        let data_bytes = serde_json::to_vec(data)?;
        let mut cursor = std::io::Cursor::new(data_bytes);
        (entry.deserialize)(Protocol::Json, &mut cursor)
    }
}

pub fn registry() -> &'static Mutex<Registry> {
    static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut registry = Registry::new();
        register_defaults(&mut registry);
        Mutex::new(registry)
    })
}

pub fn registry_guard() -> MutexGuard<'static, Registry> {
    registry().lock().expect("registry lock not poisoned")
}

pub fn serialize_tagged(value: &dyn Any, writer: &mut dyn Write) -> Result<(), Error> {
    let registry = registry_guard();
    let name = registry
        .by_type
        .get(&value.type_id())
        .ok_or_else(|| Error::UnknownType(format!("{:?}", value.type_id())))?
        .to_owned();

    let mut data = Vec::new();
    registry.serialize_erased(value, Protocol::Json, &mut data)?;
    let data: Value = serde_json::from_slice(&data)?;

    let mut obj = serde_json::Map::new();
    obj.insert("type".into(), Value::String(name.into()));
    obj.insert("data".into(), data);
    serde_json::to_writer(writer, &Value::Object(obj))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Instrument, InstrumentStore};

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
    fn registry_tagged_roundtrip() {
        let inst = aapl();
        let name = std::any::type_name::<Instrument>();
        {
            let registry = registry_guard();
            assert!(registry.is_registered(name));
        }

        let mut buf = Vec::new();
        serialize_tagged(&inst, &mut buf).unwrap();

        let back = registry_guard().deserialize_tagged(&buf).unwrap();
        let back = back.downcast::<Instrument>().unwrap();
        assert_eq!(*back, inst);
    }

    #[test]
    fn registry_unknown_type_reported() {
        let mut buf = Vec::new();
        let err = serialize_tagged(&42i32, &mut buf).unwrap_err();
        assert!(matches!(err, Error::UnknownType(_)));
    }

    #[test]
    fn registry_register_is_idempotent() {
        let mut registry = Registry::new();
        registry.register::<Instrument>().unwrap();
        registry.register::<Instrument>().unwrap();
        registry.register::<InstrumentStore>().unwrap();
        assert!(registry.is_registered(std::any::type_name::<Instrument>()));
        assert!(registry.is_registered(std::any::type_name::<InstrumentStore>()));
    }
}
