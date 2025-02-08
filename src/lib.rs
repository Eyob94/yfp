use std::{fmt::Display, future::Future};

use anyhow::anyhow;
use chrono::{Datelike, Local, NaiveDate, TimeZone, Utc};
use clap::Subcommand;
use csv::WriterBuilder;
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Response,
};
use scraper::{selectable::Selectable, Html, Selector};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt as _;
use tracing::info;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[allow(clippy::upper_case_acronyms)]
pub struct OHLC {
    date: String,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    adj_close: f64,
    volume: u64,
}

impl OHLC {
    fn insert(&mut self, sli: [f64; 6], date: String) {
        self.date = date;
        self.open = sli[0];
        self.high = sli[1];
        self.low = sli[2];
        self.close = sli[3];
        self.adj_close = sli[4];
        self.volume = sli[5] as u64;
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

fn date_to_timestamp(date_str: &str) -> anyhow::Result<i64> {
    let naive_date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")?;
    let datetime = Utc.from_utc_datetime(&naive_date.and_hms_opt(23, 59, 59).unwrap());
    Ok(datetime.timestamp())
}

pub fn parse_html(
    html: String,
    freq: Frequency,
    start: &str,
    end: Option<&str>,
) -> anyhow::Result<Vec<OHLC>> {
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

    let mut candlesticks: Vec<OHLC> = if let Some(cap) = capacity {
        Vec::with_capacity(cap as usize)
    } else {
        vec![]
    };
    for row in tbody.select(&table_row) {
        // open, high, low, close, adj.close, volume
        let mut ohlcv_vec = [0_f64; 6];
        let mut empty = true;

        let mut cells = row.select(&table_data);
        let date = cells.next().map(|d| d.inner_html()).unwrap_or_default();

        if date.is_empty() {
            continue;
        }
        for (i, data) in cells.enumerate() {
            let text = data.inner_html().replace(",", "");

            // If it fails it's the split/dividend row
            if let Ok(val) = text.parse::<f64>() {
                empty = false;
                ohlcv_vec[i] = val;
            }
        }
        if !empty {
            let mut ohlcv: OHLC = OHLC::default();
            ohlcv.insert(ohlcv_vec, date);
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

pub async fn run(
    ticker: String,
    start: String,
    end: Option<String>,
    frequency: Frequency,
    file_name: Option<String>,
    file_format: FileFormat,
) -> anyhow::Result<()> {
    let client = compose_client(&ticker, &start, end.as_deref(), frequency)?;

    let data = client.await?.text().await?;

    let data = parse_html(data, frequency, &start, end.as_deref())?;

    let file_name = if let Some(name) = file_name {
        name
    } else {
        format!(
            "yfp_{}_{}_{}_{}_{}",
            ticker,
            start,
            end.as_deref().unwrap_or("today"),
            frequency,
            Local::now().format("%Y-%m-%d")
        )
    };

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
