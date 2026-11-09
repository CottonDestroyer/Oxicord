use std::{fmt, str::FromStr, sync::atomic::{AtomicU64, Ordering}};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::serde_util::u64_from_str_or_num;

/// 2015-01-01T00:00:00Z, the epoch Discord counts snowflake time from.
const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Snowflake(pub u64);

impl Snowflake {
    pub fn timestamp_ms(self) -> u64 {
        (self.0 >> 22) + DISCORD_EPOCH_MS
    }

    pub fn datetime(self) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(self.timestamp_ms() as i64).unwrap_or_default()
    }

    /// A snowflake-shaped nonce for an optimistic send or an interaction.
    /// Discord uses it to drop a duplicate.
    pub fn nonce() -> Self {
        const DISCORD_EPOCH_MS: u128 = 1_420_070_400_000;
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        let elapsed = u64::try_from(now_ms.saturating_sub(DISCORD_EPOCH_MS)).unwrap_or(0);
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed) & 0x3F_FFFF;
        Self((elapsed << 22) | sequence)
    }
}

impl fmt::Display for Snowflake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for Snowflake {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Snowflake)
    }
}

impl Serialize for Snowflake {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Snowflake {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        u64_from_str_or_num(d).map(Snowflake)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Example straight from the public API docs.
    const DOCS_ID: Snowflake = Snowflake(175_928_847_299_117_063);

    #[test]
    fn extracts_creation_time() {
        assert_eq!(DOCS_ID.timestamp_ms(), 1_462_015_105_796);
    }

    #[test]
    fn serializes_as_string() {
        assert_eq!(serde_json::to_string(&DOCS_ID).unwrap(), "\"175928847299117063\"");
    }

    #[test]
    fn deserializes_from_string_and_number() {
        let a: Snowflake = serde_json::from_str("\"175928847299117063\"").unwrap();
        let b: Snowflake = serde_json::from_str("175928847299117063").unwrap();
        assert_eq!(a, DOCS_ID);
        assert_eq!(b, DOCS_ID);
    }

    #[test]
    fn null_becomes_none_when_optional() {
        let v: Option<Snowflake> = serde_json::from_str("null").unwrap();
        assert_eq!(v, None);
    }

    #[test]
    fn nonces_are_unique_and_snowflake_sized() {
        let (a, b) = (Snowflake::nonce(), Snowflake::nonce());
        assert_ne!(a, b);
        assert!(a.timestamp_ms() > DISCORD_EPOCH_MS);
        assert!(a.0 > 0);
    }
}
