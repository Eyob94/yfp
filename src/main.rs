#![allow(clippy::upper_case_acronyms)]
use std::fmt::Display;

use chrono::Local;
use clap::Parser;
use csv::WriterBuilder;
use tokio::io::AsyncWriteExt;
use tracing::info;
use yfp::{compose_client, human_readable_date, parse_html, Frequency};

#[derive(Parser, Debug, Clone)]
#[command(author = "Eyob", name = "yfp", about = "A yahoo finance scraper")]
pub struct Cli {
    #[arg(short = 't', long, help = "Ticker of the stock you want data for")]
    ticker: String,

    #[arg(short = 's', long, help = "Start date(Use YYYY-MM-DD)")]
    start: String,

    #[arg(
        short = 'e',
        long,
        help = "End date(If not specified, current date will be taken)"
    )]
    end: Option<String>,

    #[arg(short = 'f', long, value_enum)]
    file_format: FileFormat,

    #[arg(short = 'n', long)]
    file_name: Option<String>,

    #[command(subcommand)]
    frequency: Frequency,
}

#[derive(Clone, Debug, clap::ValueEnum)]
enum FileFormat {
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

impl Display for Cli {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let today = Local::now().format("%Y-%m-%d").to_string();
        write!(
            f,
            "\nTicker: {}\n\nStart: {}\n\nEnd: {}\n\nFrequency: {}\n\nFile Name: {}.{}\n\n",
            self.ticker,
            human_readable_date(&self.start).unwrap(),
            human_readable_date(self.end.as_deref().unwrap_or(&today)).unwrap(),
            self.frequency,
            &self
                .file_name
                .clone()
                .unwrap_or("No file specified".to_string()),
            if self.file_name.is_some() {
                match self.file_format {
                    FileFormat::CSV => "csv",
                    FileFormat::JSON => "json",
                }
            } else {
                ""
            }
        )
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().init();

    let cli = Cli::parse();

    info!("{cli}");

    let client = compose_client(&cli.ticker, &cli.start, cli.end.as_deref(), cli.frequency)?;

    let data = client.await?.text().await?;

    let data = parse_html(data, cli.frequency, &cli.start, cli.end.as_deref())?;

    let file_name = if let Some(name) = cli.file_name {
        name
    } else {
        format!(
            "yfp_{}_{}_{}_{}_{}",
            cli.ticker,
            cli.start,
            cli.end.as_deref().unwrap_or("today"),
            cli.frequency,
            Local::now().format("%Y-%m-%d")
        )
    };

    match cli.file_format {
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
    }

    Ok(())
}
