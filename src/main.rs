#![allow(clippy::upper_case_acronyms)]
use std::fmt::Display;

use chrono::Local;
use clap::Parser;
use tracing::info;
use yfp::{human_readable_date, FileFormat, Frequency};

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

    yfp::run(
        cli.ticker,
        cli.start,
        cli.end,
        cli.frequency,
        cli.file_name,
        cli.file_format,
    )
    .await
}
