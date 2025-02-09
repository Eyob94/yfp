use anyhow::anyhow;
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum Date {
    Timestamp(u64),
    Human(String),
}

impl Default for Date {
    fn default() -> Self {
        Self::Timestamp(0)
    }
}

impl Serialize for Date {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = match self {
            Date::Timestamp(ts) => {
                // Convert the timestamp to a UTC datetime and format it.
                timestamp_to_date(*ts * 1000)
                    .map_err(|_| serde::ser::Error::custom("Error converting timestamp to human"))?
            }
            Date::Human(s) => s.clone(),
        };

        serializer.serialize_str(&s)
    }
}

impl<'de> Deserialize<'de> for Date {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        let date = date_to_timestamp(&s)
            .map_err(|_| serde::de::Error::custom("Error converting date string to timestamp"))?;
        Ok(Self::Timestamp(date.max(0) as u64))
    }
}

/// Converts timestamp to date format in MM D, YYYY (Dec 28, 2005)
fn timestamp_to_date(timestamp: u64) -> anyhow::Result<String> {
    let date = DateTime::from_timestamp_millis(timestamp as i64)
        .ok_or_else(|| anyhow!("Error converting timestamp to date"))?;

    Ok(date.format("%b %-d, %Y").to_string())
}

/// Converts date format in MM D, YYYY (Dec 28, 2005) to timestamp
pub fn date_string_to_timestamp(date_str: &str) -> anyhow::Result<i64> {
    let naive_date = NaiveDate::parse_from_str(date_str, "%b %-d,%Y")?;
    let datetime = Utc.from_utc_datetime(&naive_date.and_hms_opt(0, 0, 0).unwrap());
    Ok(datetime.timestamp())
}

/// Converts date format in YYYY-MM-DD (2005-12-28) to timestamp
pub fn date_to_timestamp(date_str: &str) -> anyhow::Result<i64> {
    let naive_date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")?;
    let datetime = Utc.from_utc_datetime(&naive_date.and_hms_opt(0, 0, 0).unwrap());
    Ok(datetime.timestamp())
}

/// Converts date format in YYYY-MM-DD (2005-12-28) to human readable date (Dec 28, 2005)
pub fn human_readable_date(date_str: &str) -> anyhow::Result<String> {
    let naive_date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")?;

    let formatted_date = format!(
        "{} {}, {}",
        naive_date.format("%B"),
        naive_date.day(),
        naive_date.year()
    );

    Ok(formatted_date)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_serialization_from_timestamp() -> anyhow::Result<()> {
        let dt = Utc.with_ymd_and_hms(2005, 12, 28, 0, 0, 0).unwrap();
        let date = Date::Timestamp(dt.timestamp() as u64);
        let serialized = serde_json::to_string(&date)?;

        assert_eq!(serialized, "\"Dec 28, 2005\"");
        Ok(())
    }

    #[test]
    fn test_date_to_timestamp_valid() -> anyhow::Result<()> {
        let date_str = "2025-02-09";
        let ts = date_to_timestamp(date_str)?;
        let expected = Utc
            .with_ymd_and_hms(2025, 2, 9, 0, 0, 0)
            .map(|dt| dt.timestamp())
            .unwrap();
        assert_eq!(ts, expected);
        Ok(())
    }

    #[test]
    fn test_date_string_to_timestamp_valid() -> anyhow::Result<()> {
        let date_str = "Dec 20, 2024";
        let ts = date_string_to_timestamp(date_str)?;
        let expected = Utc
            .with_ymd_and_hms(2024, 12, 20, 0, 0, 0)
            .map(|dt| dt.timestamp())
            .unwrap();
        assert_eq!(ts, expected);
        Ok(())
    }
}
