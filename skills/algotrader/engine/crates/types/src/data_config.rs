//! Data source configuration — describes the OHLCV dataset being loaded.
//!
//! Externalizes all assumptions that previously were hardcoded per asset class:
//! benchmark symbol, excluded symbols, parquet column format, trading hours.
//! Defaults are stock-friendly (QQQ/SPY benchmarks, MultiIndex parquet, 6.5h day).
//! Crypto configs override these fields.

use serde::{Deserialize, Serialize};

/// Describes the OHLCV dataset format and asset-class-specific defaults.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct DataConfig {
    /// Benchmark/index symbol(s) for the regime filter. First found in the
    /// dataset is used. E.g. `["QQQ", "SPY"]` for stocks, `["BTCUSDT"]` for crypto.
    pub benchmark_symbols: Vec<String>,

    /// Symbols to exclude from trading (indexes, ETFs, stablecoins, etc.).
    /// Replaces the mandatory `etf_tickers.txt` file — can be specified inline.
    pub excluded_symbols: Vec<String>,

    /// Optional path to a file containing one excluded symbol per line.
    /// Merged with `excluded_symbols` when both are provided.
    pub excluded_symbols_file: Option<String>,

    /// Parquet index column name(s) to try, in order. First found is used.
    /// Defaults cover vectorbt MultiIndex (`__index_level_0__`) and timestamp.
    pub index_columns: Vec<String>,

    /// OHLCV column format (how tickers map to columns in the parquet).
    pub ohlcv_format: OhlcvFormat,

    /// Trading hours per day — used for intraday bar count calculations.
    /// 6.5 for US equities, 24.0 for crypto.
    pub trading_hours_per_day: f32,

    /// Bars per hour at the intraday resolution (12 for 5m, 2 for 30m, 1 for 1h).
    pub intraday_bars_per_hour: f32,
}

/// How tickers map to columns in the OHLCV parquet file.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum OhlcvFormat {
    /// Pandas/vectorbt MultiIndex: column names are string tuples like
    /// `"('open', 'AAPL')"`. One field-per-row, tickers in the second level.
    #[default]
    MultiIndex,
    /// Simple wide format: column names like `"AAPL_open"` or separate
    /// per-field parquet files keyed by plain ticker names.
    Wide,
}

impl Default for DataConfig {
    fn default() -> Self {
        Self {
            benchmark_symbols: vec!["QQQ".into(), "SPY".into()],
            excluded_symbols: Vec::new(),
            excluded_symbols_file: None,
            index_columns: vec![
                "__index_level_0__".into(),
                "__index_level_1__".into(),
                "timestamp".into(),
            ],
            ohlcv_format: OhlcvFormat::MultiIndex,
            trading_hours_per_day: 6.5,
            intraday_bars_per_hour: 12.0,
        }
    }
}

impl DataConfig {
    /// Crypto preset: 24/7 market, BTCUSDT benchmark, 5m bars (12/hr).
    pub fn crypto() -> Self {
        Self {
            benchmark_symbols: vec!["BTCUSDT".into()],
            trading_hours_per_day: 24.0,
            intraday_bars_per_hour: 12.0,
            ..Default::default()
        }
    }

    /// Bars per day at the intraday resolution.
    pub fn intraday_bars_per_day(&self) -> f32 {
        self.trading_hours_per_day * self.intraday_bars_per_hour
    }
}
