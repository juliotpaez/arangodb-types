use std::fmt;
use std::ops::Deref;

use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use serde::de::Visitor;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::types::dates::DBDateTime;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DBDate(pub NaiveDate);

impl DBDate {
    // CONSTRUCTORS -----------------------------------------------------------

    pub fn new(date: NaiveDate) -> Self {
        Self(date)
    }

    pub fn today() -> Self {
        Self(Utc::now().date_naive())
    }

    pub fn current_month() -> Self {
        let today = Self::today();
        Self(today.0.with_day(0).unwrap())
    }

    pub fn zero_month() -> Self {
        Self(Utc.with_ymd_and_hms(0, 1, 1, 0, 0, 0).unwrap().date_naive())
    }

    // GETTERS ----------------------------------------------------------------

    /// Checks this datetime against now as if it is an expiration.
    pub fn is_expired(&self) -> bool {
        let now = DBDate::today();
        self.0 <= now.0
    }

    pub fn months_since_zero_month(&self) -> u32 {
        let zero_month = Self::zero_month();
        (self.0.year() as u32 * 12 + self.0.month0())
            - (zero_month.0.year() as u32 * 12 + zero_month.0.month0())
    }

    // METHODS ----------------------------------------------------------------

    pub fn before_years(&self, years: u32) -> DBDate {
        DBDate(
            Utc.with_ymd_and_hms(
                self.0.year() - years as i32,
                self.0.month(),
                self.0.day(),
                0,
                0,
                0,
            )
            .unwrap()
            .date_naive(),
        )
    }

    pub fn after_days(&self, duration: u64) -> DBDate {
        DBDate(self.0 + chrono::Duration::days(duration as i64))
    }

    pub fn after_months(&self, months: u32) -> DBDate {
        let mut final_months = self.0.year() * 12;
        final_months += self.0.month0() as i32;
        final_months += months as i32;

        let year = final_months / 12;
        let month = final_months % 12;

        DBDate(
            Utc.with_ymd_and_hms(year, month as u32 + 1, self.0.day(), 0, 0, 0)
                .unwrap()
                .date_naive(),
        )
    }

    pub fn before_months(&self, months: u32) -> DBDate {
        let mut final_months = self.0.year() * 12;
        final_months += self.0.month0() as i32;
        final_months -= months as i32;

        let year = final_months / 12;
        let month = final_months % 12;

        DBDate(
            Utc.with_ymd_and_hms(year, month as u32 + 1, self.0.day(), 0, 0, 0)
                .unwrap()
                .date_naive(),
        )
    }

    pub fn to_date_time(&self) -> DBDateTime {
        DBDateTime::new(self.0.and_hms_opt(0, 0, 0).unwrap())
    }
}

impl Serialize for DBDate {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i32(self.0.num_days_from_ce())
    }
}

impl<'de> Deserialize<'de> for DBDate {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as Deserializer<'de>>::Error>
    where
        D: Deserializer<'de>,
    {
        struct DateVisitor;
        impl<'de> Visitor<'de> for DateVisitor {
            type Value = DBDate;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an integer between -2^63 and 2^63")
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(DBDate(
                    NaiveDate::from_num_days_from_ce_opt(value as i32).unwrap(),
                ))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(DBDate(
                    NaiveDate::from_num_days_from_ce_opt(value as i32).unwrap(),
                ))
            }
        }

        deserializer.deserialize_i64(DateVisitor)
    }
}

impl Deref for DBDate {
    type Target = NaiveDate;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<NaiveDate> for DBDate {
    fn from(v: NaiveDate) -> Self {
        DBDate(v)
    }
}

impl Default for DBDate {
    fn default() -> Self {
        Self::today()
    }
}

// ----------------------------------------------------------------------------
// ----------------------------------------------------------------------------
// ----------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_date() {
        let date = DBDate(
            Utc.with_ymd_and_hms(1970, 12, 7, 0, 0, 0)
                .unwrap()
                .date_naive(),
        );
        let str_date = serde_json::to_string(&date).unwrap();

        assert_eq!("719503", str_date);
        assert_eq!(date, serde_json::from_str(str_date.as_str()).unwrap());
    }

    #[test]
    fn date_after_months() {
        let original_date = DBDate(
            Utc.with_ymd_and_hms(2021, 12, 1, 0, 0, 0)
                .unwrap()
                .date_naive(),
        );
        let final_date = original_date.after_months(1);

        assert_eq!(final_date.0.year(), 2022, "The year is incorrect");
        assert_eq!(final_date.0.month(), 1, "The month is incorrect");

        let original_date = DBDate(
            Utc.with_ymd_and_hms(2021, 5, 1, 0, 0, 0)
                .unwrap()
                .date_naive(),
        );
        let final_date = original_date.after_months(20);

        assert_eq!(final_date.0.year(), 2023, "The year is incorrect");
        assert_eq!(final_date.0.month(), 1, "The month is incorrect");
    }

    #[test]
    fn date_before_months() {
        let original_date = DBDate(
            Utc.with_ymd_and_hms(2021, 1, 1, 0, 0, 0)
                .unwrap()
                .date_naive(),
        );
        let final_date = original_date.before_months(1);

        assert_eq!(final_date.0.year(), 2020, "The year is incorrect");
        assert_eq!(final_date.0.month(), 12, "The month is incorrect");

        let original_date = DBDate(
            Utc.with_ymd_and_hms(2021, 5, 1, 0, 0, 0)
                .unwrap()
                .date_naive(),
        );
        let final_date = original_date.before_months(20);

        assert_eq!(final_date.0.year(), 2019, "The year is incorrect");
        assert_eq!(final_date.0.month(), 9, "The month is incorrect");
    }
}
