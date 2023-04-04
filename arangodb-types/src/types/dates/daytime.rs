use std::fmt;
use std::ops::Deref;

use chrono::Timelike;
use serde::de::Visitor;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DBDayTime(pub chrono::NaiveTime);

impl Serialize for DBDayTime {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(self.0.num_seconds_from_midnight())
    }
}

impl<'de> Deserialize<'de> for DBDayTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        struct TimeVisitor;
        impl<'de> Visitor<'de> for TimeVisitor {
            type Value = DBDayTime;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an integer between -2^63 and 2^63")
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(DBDayTime(
                    chrono::NaiveTime::from_num_seconds_from_midnight_opt(value as u32, 0).unwrap(),
                ))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(DBDayTime(
                    chrono::NaiveTime::from_num_seconds_from_midnight_opt(value as u32, 0).unwrap(),
                ))
            }
        }

        deserializer.deserialize_u32(TimeVisitor)
    }
}

impl Deref for DBDayTime {
    type Target = chrono::NaiveTime;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<chrono::NaiveTime> for DBDayTime {
    fn from(v: chrono::NaiveTime) -> Self {
        DBDayTime(v)
    }
}

impl Default for DBDayTime {
    fn default() -> Self {
        Self(chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap())
    }
}

// ----------------------------------------------------------------------------
// ----------------------------------------------------------------------------
// ----------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_day_time() {
        let day_time = DBDayTime(chrono::NaiveTime::from_hms_opt(2, 23, 55).unwrap());
        let str_day_time = serde_json::to_string(&day_time).unwrap();

        assert_eq!("8635", str_day_time);
        assert_eq!(
            day_time,
            serde_json::from_str(str_day_time.as_str()).unwrap()
        );
    }
}
