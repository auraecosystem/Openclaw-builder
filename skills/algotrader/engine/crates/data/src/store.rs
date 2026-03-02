use std::collections::HashMap;

use strum::EnumCount;

use super::indicator::Indicator;
use engine_types::WideMatrix;

/// Shared axis metadata loaded once from the reference parquet.
pub struct Axes {
    /// Days since epoch for each row.
    pub dates: Vec<i32>,
    /// Ticker symbols; position = column index.
    pub tickers: Vec<String>,
    /// Ticker name -> column index.
    pub ticker_idx: HashMap<String, usize>,
    /// QQQ/SPY/BTCUSDT column index (for regime filter).
    pub spy_col: Option<usize>,
    /// Per-column flag: true = ETF ticker (excluded from strategies).
    pub etf_cols: Vec<bool>,
    /// Number of rows (trading days).
    pub n_rows: usize,
    /// Number of columns (tickers).
    pub n_cols: usize,
    /// Trading hours per day (6.5 for equities, 24.0 for crypto).
    pub trading_hours: f32,
}

/// Optional intraday (5-minute) data.  Replaces the 7 `Option<>` fields
/// that were scattered across the old `DataStore`.
pub struct IntradayData {
    /// OHLCV matrices for 5m bars, indexed 0..5 (same order as Indicator
    /// OHLCV variants: Open, High, Low, Close, Volume).
    pub matrices: Vec<WideMatrix>,
    /// Timestamps for each 5m row (millis since epoch).
    pub timestamps: Vec<i64>,
    /// Maps daily row index -> (start_5m_row, end_5m_row) range.
    pub day_mapping: Vec<(usize, usize)>,
    /// OHLCV matrices for 30m bars (factor=6 resampled from 5m).
    pub matrices_30m: Vec<WideMatrix>,
    /// Timestamps for 30m rows (start of each 30m period).
    pub timestamps_30m: Vec<i64>,
    /// OHLCV matrices for 1h bars (factor=12 resampled from 5m).
    pub matrices_1h: Vec<WideMatrix>,
    /// Timestamps for 1h rows (start of each 1h period).
    pub timestamps_1h: Vec<i64>,
}

/// All pre-computed indicator matrices.  Loaded once, immutable, shared via
/// `&DataStore`.
///
/// `daily` is a `Vec<WideMatrix>` with exactly `Indicator::COUNT` elements,
/// indexed by `ind as usize`.  Convenience accessors (`open()`, `high()`,
/// etc.) avoid the need to spell out the enum variant at call sites.
pub struct DataStore {
    pub axes: Axes,
    daily: Vec<WideMatrix>,
    pub intraday: Option<IntradayData>,
}

impl DataStore {
    /// Construct a new `DataStore`.  Panics if `daily.len() != Indicator::COUNT`.
    pub fn new(axes: Axes, daily: Vec<WideMatrix>, intraday: Option<IntradayData>) -> Self {
        assert_eq!(
            daily.len(),
            Indicator::COUNT,
            "DataStore::new: daily.len()={} != Indicator::COUNT={}",
            daily.len(),
            Indicator::COUNT,
        );
        Self {
            axes,
            daily,
            intraday,
        }
    }

    /// Look up the matrix for any indicator.
    #[inline(always)]
    pub fn get(&self, ind: Indicator) -> &WideMatrix {
        &self.daily[ind as usize]
    }

    // ---- OHLCV convenience accessors ----

    #[inline(always)]
    pub fn open(&self) -> &WideMatrix {
        self.get(Indicator::Open)
    }

    #[inline(always)]
    pub fn high(&self) -> &WideMatrix {
        self.get(Indicator::High)
    }

    #[inline(always)]
    pub fn low(&self) -> &WideMatrix {
        self.get(Indicator::Low)
    }

    #[inline(always)]
    pub fn close(&self) -> &WideMatrix {
        self.get(Indicator::Close)
    }

    #[inline(always)]
    pub fn volume(&self) -> &WideMatrix {
        self.get(Indicator::Volume)
    }
}
