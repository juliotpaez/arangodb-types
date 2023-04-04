use std::fmt;
use std::ops::Deref;

use chrono::{NaiveDateTime, Timelike};
use serde::de::Visitor;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::types::dates::DBDateTime;

/// A datetime stored in DB as a UNIX seconds timestamp.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DBExpiration(pub NaiveDateTime);

impl DBExpiration {
    // GETTERS ----------------------------------------------------------------

    /// Checks this datetime against now as if it is an expiration.
    pub fn is_expired(&self) -> bool {
        let now = DBDateTime::now();
        self.0 <= now.0
    }
}

impl Serialize for DBExpiration {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(self.0.timestamp())
    }
}

impl<'de> Deserialize<'de> for DBExpiration {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        struct DateTimeVisitor;
        impl<'de> Visitor<'de> for DateTimeVisitor {
            type Value = DBExpiration;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an integer between -2^63 and 2^63")
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(DBDateTime::new(NaiveDateTime::from_timestamp_opt(value, 0).unwrap()).into())
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(
                    DBDateTime::new(NaiveDateTime::from_timestamp_opt(value as i64, 0).unwrap())
                        .into(),
                )
            }
        }

        deserializer.deserialize_i64(DateTimeVisitor)
    }
}

impl Deref for DBExpiration {
    type Target = NaiveDateTime;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<DBDateTime> for DBExpiration {
    fn from(v: DBDateTime) -> Self {
        DBExpiration(v.with_nanosecond(0).unwrap())
    }
}

impl From<DBExpiration> for DBDateTime {
    fn from(v: DBExpiration) -> Self {
        DBDateTime::new(v.0)
    }
}

impl Default for DBExpiration {
    fn default() -> Self {
        DBDateTime::now().into()
    }
}
