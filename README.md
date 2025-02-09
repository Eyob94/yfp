# 📈 Yahoo Finance Scraper (`yfp`)

> **A fast, flexible, and easy-to-use CLI tool to scrape Yahoo Finance for stock data.**

This tool allows you to **fetch historical stock data** (OHLCV) from Yahoo Finance and save it in **CSV or JSON** format. It's designed to be **simple, efficient, and extendable**.

## Features

- **Scrape historical data for any ticker from Yahoo Finance**
  - **Gets OHLCV data (Open, High, Low, Close, Adj.Close, Volume)**
- **Set Start and End date**
- **Set Frequency (Daily, Weekly, Monthly)**
- **Choose file format (CSV and JSON only supported currently)**

### Installation

**Using cargo**

```
cargo install yfp
```

**Build and run from Source**

```
git clone https://Eyob94/yfp.git
cd yfp
cargo test
cargo build --release

./target/debug/yfp -h
```

### Usage

```bash
A yahoo finance scraper

Usage: yfp [OPTIONS] --ticker <TICKER> --start <START> --file-format <FILE_FORMAT> <COMMAND>

Commands:
  daily
  weekly
  monthly
  help     Print this message or the help of the given subcommand(s)

Options:
  -t, --ticker <TICKER>            Ticker of the stock you want data for
  -s, --start <START>              Start date(Use YYYY-MM-DD)
  -e, --end <END>                  End date(If not specified, current date will be taken)
  -f, --file-format <FILE_FORMAT>  [possible values: csv, json]
  -n, --file-name <FILE_NAME>
  -h, --help                       Print help

```

#### Example

```bash
yfp -t VOO -s "2011-01-01" -e "2025-01-04" -f json -n Vanguard daily

```

#### Output

```bash
2025-02-08T21:21:50.873609Z  INFO yfp:
Ticker: VOO

Start: November 26, 2001

End: November 14, 2024

Frequency: daily

File Name: Vanguard.json


2025-02-08T21:21:50.873733Z  INFO yfp: Getting historical data for VOO from November 26, 2001 until November 14, 2024 on daily frequency
2025-02-08T21:21:57.715901Z  INFO yfp: File saved to Vanguard.json
```

#### As a library

**Add it to your Cargo.toml**

```toml

yfp = "0.2"

```

**Import it**

```rust

use yfp::{retrieve_historical_data, Frequency, OHLCV};

#[tokio::main]
async fn main(){
    let voo_data: Vec<OHLCV> = retrieve_historical_data("VOO", "2020-01-01", None, Frequency::Daily).await?

    for voo_daily in voo_data.iter(){
        // voo_daily.date - in both timestamp and human readable format
        // voo_daily.open
        // voo_daily.high
        // voo_daily.low
        // voo_daily.close
        // voo_daily.adj_close
        // voo_daily.volume
    }


}

```

### Upcoming Features

- [x] Scrape Yahoo finance
- [x] Set Dates, Frequency and file formats
- [x] Publish crate to be used in other programs
- [ ] Scrape multiple tickers concurrently
  - [ ] Set different configs for each ticker
- [ ] Support output compression(`gzip` for CSV)
- [ ] Nice and more intuitive UI(TUI)

### Contributing

Contribution of any kind is welcome, but please be advised any and all contributions made to this repo is hereby applicable under the current license. Please refer to the License section for more info.

### License

This project is licensed under the MIT License. See `LICENSE` for details.
