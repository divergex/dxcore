use std::io::{Read, Write};

use super::{Error, Protocol};

pub trait Serializable {
    fn protocols() -> &'static [Protocol];

    fn serialize(&self, protocol: Protocol, writer: &mut dyn Write) -> Result<(), Error>;

    fn deserialize(protocol: Protocol, reader: &mut dyn Read) -> Result<Self, Error>
    where
        Self: Sized;
}

/// Declares a type `Serializable` and dispatches [`Serializable::serialize`]
/// to the per-protocol methods. Each listed protocol resolves to
/// `serialize_$proto`-style methods; today every protocol maps to
/// `serialize_json`/`deserialize_json`, which the type must provide.
macro_rules! serializable {
    ($ty:ty, [$($proto:ident),+ $(,)?]) => {
        impl $crate::serialization::Serializable for $ty {
            fn protocols() -> &'static [$crate::serialization::Protocol] {
                &[$( $crate::serialization::Protocol::$proto ),+]
            }

            fn serialize(
                &self,
                protocol: $crate::serialization::Protocol,
                writer: &mut dyn std::io::Write,
            ) -> Result<(), $crate::serialization::Error> {
                #[allow(unreachable_patterns)] // `_` is the extension point for future protocols
                match protocol {
                    $( $crate::serialization::Protocol::$proto => self.serialize_json(writer), )+
                    _ => Err($crate::serialization::Error::UnsupportedProtocol {
                        protocol,
                        ty: stringify!($ty),
                    }),
                }
            }

            fn deserialize(
                protocol: $crate::serialization::Protocol,
                reader: &mut dyn std::io::Read,
            ) -> Result<Self, $crate::serialization::Error> {
                #[allow(unreachable_patterns)] // `_` is the extension point for future protocols
                match protocol {
                    $( $crate::serialization::Protocol::$proto => Self::deserialize_json(reader), )+
                    _ => Err($crate::serialization::Error::UnsupportedProtocol {
                        protocol,
                        ty: stringify!($ty),
                    }),
                }
            }
        }
    };
}

macro_rules! serde_serializable {
    ($ty:ty, [$($proto:ident),+ $(,)?]) => {
        impl $ty {
            pub fn serialize_json(&self, writer: &mut dyn std::io::Write) -> Result<(), $crate::serialization::Error> {
                serde_json::to_writer(writer, self).map_err($crate::serialization::Error::Json)
            }

            pub fn deserialize_json(reader: &mut dyn std::io::Read) -> Result<Self, $crate::serialization::Error> {
                serde_json::from_reader(reader).map_err($crate::serialization::Error::Json)
            }
        }

        $crate::serialization::serializable!($ty, [$($proto),+]);
    };
}

pub(crate) use serde_serializable;
pub(crate) use serializable;
