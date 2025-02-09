use std::{borrow::Cow, fmt::Display, future::Future};

use anyhow::anyhow;
use chrono::{Local, NaiveDate};
use clap::Subcommand;
use csv::WriterBuilder;
use date_util::{date_string_to_timestamp, date_to_timestamp, human_readable_date, Date};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Response,
};
use scraper::{selectable::Selectable, Html, Selector};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt as _;
use tracing::info;

pub mod date_util;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[allow(clippy::upper_case_acronyms)]
pub struct OHLCV {
    pub date: Date,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub adj_close: f64,
    pub volume: u64,
}

impl OHLCV {
    pub fn insert(&mut self, sli: [f64; 7]) {
        self.date = Date::Timestamp(sli[0] as u64);
        self.open = sli[1];
        self.high = sli[2];
        self.low = sli[3];
        self.close = sli[4];
        self.adj_close = sli[5];
        self.volume = sli[6] as u64;
    }
}

#[derive(Debug, Clone, Copy, Subcommand)]
pub enum Frequency {
    Daily,
    Weekly,
    Monthly,
}

impl Display for Frequency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let word = match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
        };

        write!(f, "{word}")
    }
}

pub fn compose_client(
    ticker: &str,
    from: &str,
    end: Option<&str>,
    frequency: Frequency,
) -> anyhow::Result<impl Future<Output = Result<Response, reqwest::Error>> + Send> {
    let start_date = date_to_timestamp(from)?.to_string();

    let end = if let Some(end_date) = end {
        end_date.to_string()
    } else {
        Local::now().format("%Y-%m-%d").to_string()
    };

    info!(
        "Getting historical data for {ticker} from {} until {} on {} frequency",
        human_readable_date(from)?,
        human_readable_date(&end)?,
        frequency
    );

    let end_date = date_to_timestamp(&end)?.to_string();

    let base_url = format!("https://finance.yahoo.com/quote/{}/history", ticker);

    let client = reqwest::Client::new();

    let mut headers = HeaderMap::new();
    headers.insert(
        "User-Agent".parse::<HeaderName>().unwrap(),
        "Mozilla/5.0 (X11; Linux x86_64; rv:134.0) Gecko/20100101 Firefox/134.0"
            .parse::<HeaderValue>()
            .unwrap(),
    );

    let freq: String = match frequency {
        Frequency::Daily => "1d".into(),
        Frequency::Weekly => "1wk".into(),
        Frequency::Monthly => "1mo".into(),
    };

    let req = client
        .get(base_url)
        .headers(headers)
        .query(&[
            ("period1", start_date),
            ("period2", end_date),
            ("frequency", freq),
        ])
        .send();

    Ok(req)
}

pub fn parse_html(
    html: String,
    freq: Frequency,
    start: &str,
    end: Option<&str>,
) -> anyhow::Result<Vec<OHLCV>> {
    let fragment = Html::parse_fragment(&html);
    let table_body = Selector::parse("tbody").map_err(|_| anyhow!("Error parsing td"))?;
    let table_row = Selector::parse("tr").map_err(|_| anyhow!("Error parsing table row"))?;
    let table_data = Selector::parse("td").map_err(|_| anyhow!("Error parsing table data"))?;

    let today = Local::now().format("%Y-%m-%d").to_string();
    let end = end.unwrap_or_else(|| &today);

    let tbody = fragment
        .select(&table_body)
        .next()
        .ok_or_else(|| anyhow!("No tbody tag"))?;

    let capacity = get_array_size_for_frequency(freq, start, end)?;

    let mut candlesticks: Vec<OHLCV> = if let Some(cap) = capacity {
        Vec::with_capacity(cap as usize)
    } else {
        vec![]
    };
    for row in tbody.select(&table_row) {
        // open, high, low, close, adj.close, volume
        let mut ohlcv_vec = [0_f64; 7];
        let mut empty = true;

        let mut cells = row.select(&table_data);
        let date: f64 = cells
            .next()
            .map(|d| {
                let date_in_text = d.inner_html();
                date_string_to_timestamp(&date_in_text).unwrap_or_default() as f64
            })
            .unwrap_or_default();

        if date == 0_f64 {
            continue;
        } else {
            ohlcv_vec[0] = date;
        }

        for (i, data) in cells.enumerate() {
            let text = data.inner_html().replace(",", "");

            // If it fails it's the split/dividend row
            if let Ok(val) = text.parse::<f64>() {
                empty = false;
                ohlcv_vec[i + 1] = val;
            }
        }
        if !empty {
            let mut ohlcv: OHLCV = OHLCV::default();
            ohlcv.insert(ohlcv_vec);
            candlesticks.push(ohlcv);
        }
    }

    Ok(candlesticks)
}

fn get_array_size_for_frequency(
    freq: Frequency,
    start: &str,
    end: &str,
) -> anyhow::Result<Option<u64>> {
    let start = NaiveDate::parse_from_str(start, "%Y-%m-%d")?;
    let end = NaiveDate::parse_from_str(end, "%Y-%m-%d")?;

    let num = match freq {
        Frequency::Daily => Some((end - start).num_days().max(0) as u64),
        Frequency::Weekly => Some((end - start).num_weeks().max(0) as u64),
        // Can't accurately calculate months
        Frequency::Monthly => None,
    };

    Ok(num)
}

#[derive(Clone, Debug, clap::ValueEnum)]
pub enum FileFormat {
    CSV,
    JSON,
}

impl Display for FileFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let format = match self {
            Self::CSV => "csv",
            Self::JSON => "json",
        };

        write!(f, "{format}")
    }
}

pub async fn retrieve_historical_data(
    ticker: &str,
    start: &str,
    end: Option<&str>,
    frequency: Frequency,
) -> anyhow::Result<Vec<OHLCV>> {
    let client = compose_client(ticker, start, end, frequency)?;

    let data = client.await?.text().await?;

    let parsed_data = parse_html(data, frequency, start, end)?;

    Ok(parsed_data)
}

pub fn prepare_file_name<'a>(
    ticker: &'a str,
    start: &'a str,
    end: Option<&'a str>,
    frequency: Frequency,
    file_name: Option<&'a str>,
) -> Cow<'a, str> {
    if let Some(name) = file_name {
        Cow::Borrowed(name)
    } else {
        let autoname = format!(
            "yfp_{}_{}_{}_{}_{}",
            ticker,
            start,
            end.unwrap_or("today"),
            frequency,
            Local::now().format("%Y-%m-%d")
        );
        Cow::Owned(autoname)
    }
}

pub async fn add_to_file(
    data: Vec<OHLCV>,
    file_name: &str,
    file_format: FileFormat,
) -> anyhow::Result<()> {
    match file_format {
        FileFormat::CSV => {
            let mut buf = Vec::new();

            // drop exclusive reference in scope
            {
                let mut wtr = WriterBuilder::new().from_writer(&mut buf);

                for record in data {
                    wtr.serialize(record)?;
                }

                wtr.flush()?;
            }

            let mut file = tokio::fs::File::options()
                .create(true)
                .truncate(true)
                .write(true)
                .open(format!("{}.csv", file_name))
                .await?;

            file.write_all(&buf).await?;
            info!("File saved to {file_name}.csv");
        }
        FileFormat::JSON => {
            let serialized_data = serde_json::to_string_pretty(&data)?;

            let mut file = tokio::fs::File::options()
                .create(true)
                .truncate(true)
                .write(true)
                .open(format!("{}.json", file_name))
                .await?;

            file.write_all(serialized_data.as_bytes()).await?;
            info!("File saved to {file_name}.json");
        }
    };

    Ok(())
}

#[cfg(test)]
mod test {
    use core::f64;

    use date_util::Date;
    use tempfile::tempdir;
    use tokio::fs;

    use super::*;

    #[test]
    fn test_proper_naming() {
        let name = prepare_file_name(
            "VOO",
            "2020-01-01",
            Some("2024-01-01"),
            Frequency::Daily,
            None,
        );

        assert!(name.starts_with("yfp_VOO_2020-01-01_2024-01-01_daily_"));

        let given_name = prepare_file_name(
            "VOO",
            "2020-01-01",
            Some("2024-01-01"),
            Frequency::Daily,
            Some("proper_name"),
        );

        assert_eq!(given_name, "proper_name");
    }

    #[tokio::test]
    async fn test_get_historical_data() -> anyhow::Result<()> {
        let ticker = "VOO";
        let parsed_data =
            retrieve_historical_data(ticker, "2020-01-01", None, Frequency::Monthly).await?;
        assert!(!parsed_data.is_empty());

        // Closing price of VOO on January of 2020 was 273.59
        assert!((parsed_data.last().unwrap().adj_close - 273.59).abs() < f64::EPSILON);

        Ok(())
    }

    #[tokio::test]
    async fn test_add_file_csv() -> anyhow::Result<()> {
        let temp_dir = tempdir()?;
        let base_path = temp_dir.path().join("test_csv");
        let file_name = base_path.to_str().unwrap();

        let data = vec![
            OHLCV {
                date: Date::Human("Dec 24, 2020".into()),
                open: 1.0,
                high: 2.0,
                low: 0.5,
                close: 1.5,
                adj_close: 1.5,
                volume: 100,
            },
            OHLCV {
                date: Date::Human("Dec 25, 2020".into()),
                open: 1.5,
                high: 2.5,
                low: 1.0,
                close: 2.0,
                adj_close: 2.0,
                volume: 150,
            },
        ];

        add_to_file(data, file_name, FileFormat::CSV).await?;

        let file_path = base_path.with_extension("csv");
        let content = fs::read_to_string(&file_path).await?;
        assert!(!content.trim().is_empty());

        assert!(content.contains("Dec 24, 2020"));
        assert!(content.contains("1.0"));
        assert!(content.contains("1.5"));

        Ok(())
    }
}
