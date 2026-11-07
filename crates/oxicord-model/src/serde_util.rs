use std::fmt;

use serde::{Deserializer, de};

/// Discord sends 64 bit integers as strings so JavaScript can hold them, but
/// a few payloads use plain numbers. Take either.
pub(crate) fn u64_from_str_or_num<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    struct Visitor;

    impl de::Visitor<'_> for Visitor {
        type Value = u64;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("an unsigned integer or a numeric string")
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> Result<u64, E> {
            Ok(v)
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<u64, E> {
            v.parse().map_err(E::custom)
        }
    }

    d.deserialize_any(Visitor)
}

/// A list where an entry we cannot read is dropped instead of failing the
/// whole payload. For optional enrichment that must never block a login. A null
/// reads as empty.
pub(crate) fn lenient_vec<'de, D, T>(d: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    use serde::Deserialize as _;
    let raw = Option::<Vec<serde_json::Value>>::deserialize(d)?.unwrap_or_default();
    Ok(raw.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect())
}

/// A value we cannot read becomes its default instead of failing the payload.
pub(crate) fn lenient_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    use serde::Deserialize as _;
    let raw = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(raw).unwrap_or_default())
}
