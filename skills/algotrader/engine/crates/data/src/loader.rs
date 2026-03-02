use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use polars::prelude::*;
use rayon::prelude::*;
use strum::{EnumCount, IntoEnumIterator};

use engine_types::WideMatrix;

use super::indicator::Indicator;
use super::store::{Axes, DataStore, IntradayData};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Load all daily + optional 5m data into a `DataStore`.
///
/// Layout on disk:
///   `data_dir/ohlcv.parquet`       -- MultiIndex OHLCV
///   `data_dir/cache/<ind>.parquet`  -- one file per non-OHLCV indicator
///   `data_dir/ohlcv_5m.parquet`    -- optional intraday bars
///   `data_dir/etf_tickers.txt`     -- optional ETF exclusion list
///
/// Indicator cache files are loaded in parallel via rayon.
pub fn load_data_store(data_dir: &Path) -> Result<DataStore> {
    let cache_dir = data_dir.join("cache");

    // Use the first available cache file to establish the canonical axes
    // (row count, column order, dates).
    let ref_path = cache_dir.join("sma_10.parquet");
    let dates = extract_dates(&ref_path)?;
    let (tickers, _) = load_wide_parquet(&ref_path)?;
    let n_rows = dates.len();
    let n_cols = tickers.len();

    eprintln!("  Axes: {} rows x {} tickers", n_rows, n_cols);

    // -- Build Axes ----------------------------------------------------------

    let ticker_idx: HashMap<String, usize> = tickers
        .iter()
        .enumerate()
        .map(|(i, t)| (t.clone(), i))
        .collect();

    // QQQ preferred for regime filter (per Qullamaggie rules), fall back to
    // SPY/BTCUSDT.
    let spy_col = ticker_idx
        .get("QQQ")
        .or_else(|| ticker_idx.get("SPY"))
        .or_else(|| ticker_idx.get("BTCUSDT"))
        .copied();

    let etf_set = load_etf_set(data_dir);
    let etf_cols: Vec<bool> = tickers
        .iter()
        .map(|t| etf_set.contains(&t.to_uppercase()))
        .collect();

    // -- Load indicator cache files in parallel via Indicator::iter() --------

    // Collect the non-OHLCV indicators that have cache files.
    let cacheable: Vec<Indicator> = Indicator::iter()
        .filter(|ind| ind.cache_filename().is_some())
        .collect();

    let loaded: Vec<(Indicator, WideMatrix)> = cacheable
        .par_iter()
        .map(|&ind| {
            let filename = ind.cache_filename().expect("filtered for Some");
            let path = cache_dir.join(filename);
            let (_, mat) = load_wide_parquet(&path)
                .unwrap_or_else(|e| panic!("Failed to load {}: {}", filename, e));
            assert_eq!(mat.n_rows(), n_rows, "{:?} row count mismatch", ind);
            assert_eq!(mat.n_cols(), n_cols, "{:?} col count mismatch", ind);
            (ind, mat)
        })
        .collect();

    let mut indicator_map: HashMap<u8, WideMatrix> = loaded
        .into_iter()
        .map(|(ind, mat)| (ind as u8, mat))
        .collect();

    // -- Load OHLCV from the combined parquet --------------------------------

    eprintln!("  Loading OHLCV parquet...");
    let ohlcv_path = data_dir.join("ohlcv.parquet");
    let [open, high, low, close, volume] = load_ohlcv(&ohlcv_path, &tickers)?;

    // Insert OHLCV matrices into the map so we can build the Vec in order.
    indicator_map.insert(Indicator::Open as u8, open);
    indicator_map.insert(Indicator::High as u8, high);
    indicator_map.insert(Indicator::Low as u8, low);
    indicator_map.insert(Indicator::Close as u8, close);
    indicator_map.insert(Indicator::Volume as u8, volume);

    // -- Assemble daily Vec in enum-discriminant order -----------------------

    let mut daily: Vec<WideMatrix> = Vec::with_capacity(Indicator::COUNT);
    for ind in Indicator::iter() {
        let mat = indicator_map
            .remove(&(ind as u8))
            .unwrap_or_else(|| panic!("Missing matrix for {:?}", ind));
        daily.push(mat);
    }

    // -- Optionally load 5m intraday data ------------------------------------

    let fivemin_path = data_dir.join("ohlcv_5m.parquet");
    let intraday = if fivemin_path.exists() {
        eprintln!("  Loading 5m parquet...");
        let [o5, h5, l5, c5, v5] = load_ohlcv(&fivemin_path, &tickers)?;
        let timestamps = extract_5m_timestamps(&fivemin_path)?;
        let day_mapping = build_day_to_5m(&dates, &timestamps);
        eprintln!("  5m data: {} rows x {} cols", o5.n_rows(), o5.n_cols());
        Some(IntradayData {
            matrices: vec![o5, h5, l5, c5, v5],
            timestamps,
            day_mapping,
        })
    } else {
        None
    };

    // -- Assemble DataStore --------------------------------------------------

    let axes = Axes {
        dates,
        tickers,
        ticker_idx,
        spy_col,
        etf_cols,
        n_rows,
        n_cols,
    };

    Ok(DataStore::new(axes, daily, intraday))
}

/// Resolve a date string ("2020-01-01") to the first row index whose date
/// is >= the target.  Returns `None` if the date cannot be parsed or lies
/// beyond the last row.
pub fn resolve_date_row(axes: &Axes, date_str: &str) -> Option<usize> {
    let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()?;
    let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1)?;
    let target_days = (date - epoch).num_days() as i32;
    axes.dates.iter().position(|&d| d >= target_days)
}

// ---------------------------------------------------------------------------
// Private helpers (all carried over from the original data.rs)
// ---------------------------------------------------------------------------

/// Load a wide-format parquet (DatetimeIndex x tickers, f64) into a
/// `WideMatrix` (f32).  Returns `(tickers, matrix)`.
fn load_wide_parquet(path: &Path) -> Result<(Vec<String>, WideMatrix)> {
    let df = LazyFrame::scan_parquet(path, Default::default())?
        .collect()
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let col_names: Vec<String> = df
        .get_column_names_str()
        .iter()
        .map(|s| s.to_string())
        .collect();

    // Filter out the pandas index column (stock: __index_level_0__, crypto:
    // timestamp).
    let index_names = ["__index_level_0__", "timestamp"];
    let tickers: Vec<String> = col_names
        .iter()
        .filter(|c| !index_names.contains(&c.as_str()) && !c.starts_with("__index_level_"))
        .cloned()
        .collect();

    let n_rows = df.height();
    let n_cols = tickers.len();
    let mut data = vec![f32::NAN; n_rows * n_cols];

    for (ci, ticker) in tickers.iter().enumerate() {
        let series = df
            .column(ticker.as_str())
            .with_context(|| format!("Column '{}' not found in {}", ticker, path.display()))?;

        // Handle f64, f32, and integer dtypes
        if let Ok(ca) = series.f64() {
            for (ri, opt_val) in ca.into_iter().enumerate() {
                data[ri * n_cols + ci] = opt_val.map(|v| v as f32).unwrap_or(f32::NAN);
            }
        } else if let Ok(ca) = series.f32() {
            for (ri, opt_val) in ca.into_iter().enumerate() {
                data[ri * n_cols + ci] = opt_val.unwrap_or(f32::NAN);
            }
        } else if let Ok(ca) = series.i32() {
            for (ri, opt_val) in ca.into_iter().enumerate() {
                data[ri * n_cols + ci] = opt_val.map(|v| v as f32).unwrap_or(f32::NAN);
            }
        } else if let Ok(ca) = series.i64() {
            for (ri, opt_val) in ca.into_iter().enumerate() {
                data[ri * n_cols + ci] = opt_val.map(|v| v as f32).unwrap_or(f32::NAN);
            }
        } else {
            anyhow::bail!(
                "Column '{}' has unsupported dtype in {}",
                ticker,
                path.display()
            );
        }
    }

    Ok((tickers, WideMatrix::new(data, n_rows, n_cols)))
}

/// Extract dates as days-since-epoch from the index column of a parquet.
fn extract_dates(path: &Path) -> Result<Vec<i32>> {
    let df = LazyFrame::scan_parquet(path, Default::default())?.collect()?;

    let col_names: Vec<String> = df
        .get_column_names_str()
        .iter()
        .map(|s| s.to_string())
        .collect();

    let index_col = col_names
        .iter()
        .find(|c| c.starts_with("__index_level_") || c.as_str() == "timestamp")
        .context("No index column found in parquet (expected __index_level_* or timestamp)")?;

    let series = df.column(index_col.as_str())?;

    // Try datetime first, then date
    if let Ok(ca) = series.datetime() {
        // milliseconds since epoch -> days
        Ok(ca
            .into_iter()
            .map(|opt| opt.map(|ms| (ms / 86_400_000) as i32).unwrap_or(0))
            .collect())
    } else if let Ok(ca) = series.date() {
        Ok(ca.into_iter().map(|opt| opt.unwrap_or(0)).collect())
    } else {
        // Fallback: row indices
        Ok((0..df.height() as i32).collect())
    }
}

/// Load the 5 OHLCV fields from a MultiIndex parquet whose column names
/// look like `('open', 'GTX')`.
fn load_ohlcv(path: &Path, canonical_tickers: &[String]) -> Result<[WideMatrix; 5]> {
    let df = LazyFrame::scan_parquet(path, Default::default())?
        .collect()
        .with_context(|| format!("Failed to read OHLCV: {}", path.display()))?;

    let n_rows = df.height();
    let n_cols = canonical_tickers.len();
    let ticker_idx: HashMap<&str, usize> = canonical_tickers
        .iter()
        .enumerate()
        .map(|(i, t)| (t.as_str(), i))
        .collect();

    let fields = ["open", "high", "low", "close", "volume"];
    let mut matrices: Vec<Vec<f32>> = fields
        .iter()
        .map(|_| vec![f32::NAN; n_rows * n_cols])
        .collect();

    let col_names: Vec<String> = df
        .get_column_names_str()
        .iter()
        .map(|s| s.to_string())
        .collect();

    for col_name in &col_names {
        // Parse "('field', 'ticker')" format
        let trimmed = col_name.trim_matches(|c: char| c == '(' || c == ')' || c == ' ');
        let parts: Vec<&str> = trimmed.split(',').collect();
        if parts.len() != 2 {
            continue;
        }
        let field = parts[0].trim().trim_matches('\'');
        let ticker = parts[1].trim().trim_matches('\'');

        let field_idx = match fields.iter().position(|f| *f == field) {
            Some(i) => i,
            None => continue,
        };
        let col_idx = match ticker_idx.get(ticker) {
            Some(&i) => i,
            None => continue,
        };

        let series = df.column(col_name.as_str())?;
        if let Ok(ca) = series.f32() {
            for (ri, opt_val) in ca.into_iter().enumerate() {
                matrices[field_idx][ri * n_cols + col_idx] = opt_val.unwrap_or(f32::NAN);
            }
        } else if let Ok(ca) = series.f64() {
            for (ri, opt_val) in ca.into_iter().enumerate() {
                matrices[field_idx][ri * n_cols + col_idx] =
                    opt_val.map(|v| v as f32).unwrap_or(f32::NAN);
            }
        }
    }

    let [open, high, low, close, volume] =
        [0, 1, 2, 3, 4].map(|i| WideMatrix::new(std::mem::take(&mut matrices[i]), n_rows, n_cols));

    Ok([open, high, low, close, volume])
}

/// Load ETF ticker set from `etf_tickers.txt` (one ticker per line).
fn load_etf_set(data_dir: &Path) -> HashSet<String> {
    let path = data_dir.join("etf_tickers.txt");
    match fs::read_to_string(&path) {
        Ok(content) => content.lines().map(|l| l.trim().to_uppercase()).collect(),
        Err(_) => HashSet::new(),
    }
}

/// Extract 5m timestamps as millis-since-epoch from the index column.
fn extract_5m_timestamps(path: &Path) -> Result<Vec<i64>> {
    let df = LazyFrame::scan_parquet(path, Default::default())?.collect()?;

    let col_names: Vec<String> = df
        .get_column_names_str()
        .iter()
        .map(|s| s.to_string())
        .collect();

    let index_col = col_names
        .iter()
        .find(|c| c.starts_with("__index_level_") || c.as_str() == "timestamp")
        .context("No index column found in 5m parquet")?;

    let series = df.column(index_col.as_str())?;

    if let Ok(ca) = series.datetime() {
        Ok(ca.into_iter().map(|opt| opt.unwrap_or(0)).collect())
    } else if let Ok(ca) = series.i64() {
        Ok(ca.into_iter().map(|opt| opt.unwrap_or(0)).collect())
    } else {
        anyhow::bail!("5m index column has unsupported dtype");
    }
}

/// Build the daily-row -> 5m-row-range mapping.
///
/// For each daily row, finds the contiguous range of 5m rows whose
/// timestamp falls within that calendar day.
fn build_day_to_5m(daily_dates: &[i32], ts_5m: &[i64]) -> Vec<(usize, usize)> {
    let n_daily = daily_dates.len();
    let n_5m = ts_5m.len();
    let mut mapping = vec![(0usize, 0usize); n_daily];

    if n_5m == 0 {
        return mapping;
    }

    let ms_per_day: i64 = 86_400_000;
    let mut cursor = 0usize;

    for (di, &day) in daily_dates.iter().enumerate() {
        let day_start_ms = day as i64 * ms_per_day;
        let day_end_ms = day_start_ms + ms_per_day;

        // Advance cursor to first 5m bar on or after this day
        while cursor < n_5m && ts_5m[cursor] < day_start_ms {
            cursor += 1;
        }
        let start = cursor;

        // Find end of this day's 5m bars
        let mut end = start;
        while end < n_5m && ts_5m[end] < day_end_ms {
            end += 1;
        }

        mapping[di] = (start, end);
    }

    mapping
}
