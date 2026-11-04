use std::fmt;

use serde::{
    Serializer,
    de::{self, Deserializer, Visitor},
};

// TOML integers are signed; quoted seeds also cover the entire u64 entropy range.
pub(super) fn serialize_seed<S: Serializer>(
    seed: &Option<u64>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match seed {
        Some(seed) => serializer.serialize_str(&seed.to_string()),
        None => serializer.serialize_none(),
    }
}

pub(super) fn deserialize_seed<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<u64>, D::Error> {
    struct Seed;
    impl Visitor<'_> for Seed {
        type Value = u64;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a nonnegative integer or a quoted u64 seed")
        }

        fn visit_u64<E: de::Error>(self, value: u64) -> Result<u64, E> {
            Ok(value)
        }

        fn visit_i64<E: de::Error>(self, value: i64) -> Result<u64, E> {
            value.try_into().map_err(E::custom)
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<u64, E> {
            value.parse().map_err(E::custom)
        }
    }
    deserializer.deserialize_any(Seed).map(Some)
}
