use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use polars::prelude::*;
use rayon::prelude::*;
use strum::{EnumCount, IntoEnumIterator};

use engine_types::{DataConfig, OhlcvFormat, WideMatrix};

use super::compute;
use super::indicator::Indicator;
use super::resample;
use super::store::{Axes, DataStore, IntradayData};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Load all daily + optional 5m data into a `DataStore`.
///
/// Layout on disk:
///   `data_dir/ohlcv.parquet`       -- OHLCV (MultiIndex or Wide format)
///   `data_dir/cache/<ind>.parquet`  -- one file per cached indicator
///   `data_dir/ohlcv_5m.parquet`    -- optional intraday bars
///
/// Axes (dates, tickers) are derived from `ohlcv.parquet` directly — no
/// reference cache file required. Cache files are loaded in parallel; a
/// missing cache file emits a warning and fills with NaN (graceful degrade).
pub fn load_data_store(data_dir: &Path, data_config: &DataConfig) -> Result<DataStore> {
    let ohlcv_path = data_dir.join("ohlcv.parquet");

    // -- A5: Derive axes from ohlcv.parquet (not from a cache file) ----------

    eprintln!("  Deriving axes from OHLCV parquet...");
    let dates = extract_dates_configurable(&ohlcv_path, &data_config.index_columns)?;
    let tickers = discover_tickers(&ohlcv_path, &data_config.ohlcv_format)?;
    let n_rows = dates.len();
    let n_cols = tickers.len();

    eprintln!("  Axes: {} rows x {} tickers", n_rows, n_cols);

    // -- A2: Configurable benchmark lookup -----------------------------------

    let ticker_idx: HashMap<String, usize> = tickers
        .iter()
        .enumerate()
        .map(|(i, t)| (t.clone(), i))
        .collect();

    let (spy_col, benchmark_name) = data_config
        .benchmark_symbols
        .iter()
        .find_map(|sym| ticker_idx.get(sym.as_str()).copied().map(|c| (c, sym.clone())))
        .map(|(c, s)| (Some(c), s))
        .unwrap_or((None, String::new()));

    if let Some(col) = spy_col {
        eprintln!("  Benchmark: {} (col {})", benchmark_name, col);
    } else {
        eprintln!(
            "  Warning: no benchmark symbol found ({:?}); regime filter disabled",
            data_config.benchmark_symbols
        );
    }

    // -- A3: Excluded symbols from config + optional file --------------------

    let etf_set = load_excluded_set(data_dir, data_config);
    let etf_cols: Vec<bool> = tickers
        .iter()
        .map(|t| etf_set.contains(&t.to_uppercase()))
        .collect();

    let n_excluded = etf_cols.iter().filter(|&&b| b).count();
    if n_excluded > 0 {
        eprintln!("  Excluding {} symbols from trading", n_excluded);
    }

    // -- Load indicator cache files in parallel (graceful on missing) --------

    let cache_dir = data_dir.join("cache");

    let cacheable: Vec<Indicator> = Indicator::iter()
        .filter(|ind| ind.cache_filename().is_some())
        .collect();

    let loaded: Vec<(Indicator, WideMatrix)> = cacheable
        .par_iter()
        .filter_map(|&ind| {
            let filename = ind.cache_filename().expect("filtered for Some");
            let path = cache_dir.join(filename);
            if !path.exists() {
                eprintln!("  Warning: cache missing for {:?} — filling with NaN", ind);
                let mat = WideMatrix::new(vec![f32::NAN; n_rows * n_cols], n_rows, n_cols);
                return Some((ind, mat));
            }
            match load_wide_parquet(&path, &data_config.index_columns) {
                Ok((_, mat)) => {
                    if mat.n_rows() != n_rows || mat.n_cols() != n_cols {
                        eprintln!(
                            "  Warning: {:?} shape {}×{} != axes {}×{} — filling with NaN",
                            ind,
                            mat.n_rows(),
                            mat.n_cols(),
                            n_rows,
                            n_cols
                        );
                        Some((ind, WideMatrix::new(vec![f32::NAN; n_rows * n_cols], n_rows, n_cols)))
                    } else {
                        Some((ind, mat))
                    }
                }
                Err(e) => {
                    eprintln!("  Warning: failed to load {:?} — {}", ind, e);
                    Some((ind, WideMatrix::new(vec![f32::NAN; n_rows * n_cols], n_rows, n_cols)))
                }
            }
        })
        .collect();

    let mut indicator_map: HashMap<u8, WideMatrix> = loaded
        .into_iter()
        .map(|(ind, mat)| (ind as u8, mat))
        .collect();

    // -- Load OHLCV ----------------------------------------------------------

    eprintln!("  Loading OHLCV parquet...");
    let [open, high, low, close, volume] =
        load_ohlcv(&ohlcv_path, &tickers, &data_config.ohlcv_format)?;

    // -- Compute runtime indicators from OHLCV (no cache files needed) -------

    eprintln!("  Computing runtime indicators...");
    let [sma10, sma20, vol_sma20, atr14, ret21, ret63, ret126, pct10d, dist52w, consol_high, consec_green] =
        compute::compute_column_local(&open, &high, &low, &close, &volume);

    eprintln!("  Computing cross-sectional RS percentile ranks...");
    let [rs_1m, rs_3m, rs_6m] = compute::compute_rs_pctrank(&ret21, &ret63, &ret126);

    // Derived indicators for filter fusion (computed before moving base indicators)
    let adr_pct_mat = compute::adr_pct(&atr14, &close);
    let extension_atr_mat = compute::extension_atr(&close, &consol_high, &atr14);

    indicator_map.insert(Indicator::Sma10 as u8, sma10);
    indicator_map.insert(Indicator::Sma20 as u8, sma20);
    indicator_map.insert(Indicator::VolSma20 as u8, vol_sma20);
    indicator_map.insert(Indicator::Atr14 as u8, atr14);
    indicator_map.insert(Indicator::Ret21 as u8, ret21);
    indicator_map.insert(Indicator::Ret63 as u8, ret63);
    indicator_map.insert(Indicator::Ret126 as u8, ret126);
    indicator_map.insert(Indicator::Pct10d as u8, pct10d);
    indicator_map.insert(Indicator::Dist52w as u8, dist52w);
    indicator_map.insert(Indicator::ConsolHigh as u8, consol_high);
    indicator_map.insert(Indicator::ConsecGreen as u8, consec_green);
    indicator_map.insert(Indicator::RsPctrank1m as u8, rs_1m);
    indicator_map.insert(Indicator::RsPctrank3m as u8, rs_3m);
    indicator_map.insert(Indicator::RsPctrank6m as u8, rs_6m);
    indicator_map.insert(Indicator::AdrPct as u8, adr_pct_mat);
    indicator_map.insert(Indicator::ExtensionAtr as u8, extension_atr_mat);

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
        let [o5, h5, l5, c5, v5] =
            load_ohlcv(&fivemin_path, &tickers, &data_config.ohlcv_format)?;
        let timestamps = extract_5m_timestamps(&fivemin_path, &data_config.index_columns)?;
        let day_mapping = build_day_to_5m(&dates, &timestamps);
        eprintln!("  5m data: {} rows x {} cols", o5.n_rows(), o5.n_cols());

        let [o30, h30, l30, c30, v30] = resample::resample_ohlcv(&o5, &h5, &l5, &c5, &v5, 6);
        let ts_30m = resample::resample_timestamps(&timestamps, 6);
        let [o1h, h1h, l1h, c1h, v1h] = resample::resample_ohlcv(&o5, &h5, &l5, &c5, &v5, 12);
        let ts_1h = resample::resample_timestamps(&timestamps, 12);

        Some(IntradayData {
            matrices: vec![o5, h5, l5, c5, v5],
            timestamps,
            day_mapping,
            matrices_30m: vec![o30, h30, l30, c30, v30],
            timestamps_30m: ts_30m,
            matrices_1h: vec![o1h, h1h, l1h, c1h, v1h],
            timestamps_1h: ts_1h,
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
        trading_hours: data_config.trading_hours_per_day,
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
// A3: Excluded symbols loader
// ---------------------------------------------------------------------------

/// Build the excluded symbol set from DataConfig inline list + optional file.
fn load_excluded_set(data_dir: &Path, data_config: &DataConfig) -> HashSet<String> {
    let mut set: HashSet<String> = data_config
        .excluded_symbols
        .iter()
        .map(|s| s.to_uppercase())
        .collect();

    // Optional file path (relative to data_dir if not absolute)
    let file_path = data_config
        .excluded_symbols_file
        .as_deref()
        .map(|p| {
            let p = Path::new(p);
            if p.is_absolute() { p.to_path_buf() } else { data_dir.join(p) }
        })
        // Fall back to legacy etf_tickers.txt for backward compatibility
        .unwrap_or_else(|| data_dir.join("etf_tickers.txt"));

    if let Ok(content) = fs::read_to_string(&file_path) {
        for line in content.lines() {
            let sym = line.trim().to_uppercase();
            if !sym.is_empty() {
                set.insert(sym);
            }
        }
    }

    set
}

// ---------------------------------------------------------------------------
// A4/A5: Ticker discovery and date extraction
// ---------------------------------------------------------------------------

/// Discover the ordered ticker list from an OHLCV parquet file.
///
/// MultiIndex: parses `"('open', 'AAPL')"` columns, returns tickers for field "open".
/// Wide: strips the `"_open"` suffix from `"AAPL_open"` columns.
fn discover_tickers(path: &Path, format: &OhlcvFormat) -> Result<Vec<String>> {
    let df = LazyFrame::scan_parquet(path, Default::default())?
        .collect()
        .with_context(|| format!("Failed to scan {}", path.display()))?;

    let col_names: Vec<String> = df
        .get_column_names_str()
        .iter()
        .map(|s| s.to_string())
        .collect();

    let tickers = match format {
        OhlcvFormat::MultiIndex => {
            let mut seen = Vec::new();
            let mut visited = HashSet::new();
            for name in &col_names {
                let trimmed = name.trim_matches(|c: char| c == '(' || c == ')' || c == ' ');
                let parts: Vec<&str> = trimmed.split(',').collect();
                if parts.len() == 2 {
                    let field = parts[0].trim().trim_matches('\'');
                    let ticker = parts[1].trim().trim_matches('\'');
                    if field == "open" && visited.insert(ticker.to_string()) {
                        seen.push(ticker.to_string());
                    }
                }
            }
            seen
        }
        OhlcvFormat::Wide => {
            let mut seen = Vec::new();
            let mut visited = HashSet::new();
            for name in &col_names {
                if let Some(ticker) = name.strip_suffix("_open") {
                    if visited.insert(ticker.to_string()) {
                        seen.push(ticker.to_string());
                    }
                }
            }
            seen
        }
    };

    anyhow::ensure!(!tickers.is_empty(), "No tickers found in {}", path.display());
    Ok(tickers)
}

/// Extract dates as days-since-epoch from the index column of a parquet.
/// Uses `index_columns` list to find the column, in order.
fn extract_dates_configurable(path: &Path, index_columns: &[String]) -> Result<Vec<i32>> {
    let df = LazyFrame::scan_parquet(path, Default::default())?.collect()?;

    let col_names: Vec<String> = df
        .get_column_names_str()
        .iter()
        .map(|s| s.to_string())
        .collect();

    // Try configured index columns first, then fall back to prefix detection
    let index_col = index_columns
        .iter()
        .find(|c| col_names.contains(c))
        .cloned()
        .or_else(|| {
            col_names
                .iter()
                .find(|c| c.starts_with("__index_level_") || c.as_str() == "timestamp")
                .cloned()
        });

    let Some(index_col) = index_col else {
        // No date index found — use row indices
        return Ok((0..df.height() as i32).collect());
    };

    let series = df.column(index_col.as_str())?;

    if let Ok(ca) = series.datetime() {
        // milliseconds since epoch -> days
        Ok(ca
            .into_iter()
            .map(|opt| opt.map(|ms| (ms / 86_400_000) as i32).unwrap_or(0))
            .collect())
    } else if let Ok(ca) = series.date() {
        Ok(ca.into_iter().map(|opt| opt.unwrap_or(0)).collect())
    } else {
        Ok((0..df.height() as i32).collect())
    }
}

// ---------------------------------------------------------------------------
// Parquet loaders
// ---------------------------------------------------------------------------

/// Load a wide-format cache parquet (simple ticker columns) into a WideMatrix.
/// Index column is identified using the configured column names.
fn load_wide_parquet(path: &Path, index_columns: &[String]) -> Result<(Vec<String>, WideMatrix)> {
    let df = LazyFrame::scan_parquet(path, Default::default())?
        .collect()
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let col_names: Vec<String> = df
        .get_column_names_str()
        .iter()
        .map(|s| s.to_string())
        .collect();

    // Exclude index columns and pandas index level columns
    let is_index = |c: &str| {
        c.starts_with("__index_level_")
            || c == "timestamp"
            || index_columns.iter().any(|ic| ic == c)
    };

    let tickers: Vec<String> = col_names
        .iter()
        .filter(|c| !is_index(c.as_str()))
        .cloned()
        .collect();

    let n_rows = df.height();
    let n_cols = tickers.len();
    let mut data = vec![f32::NAN; n_rows * n_cols];

    for (ci, ticker) in tickers.iter().enumerate() {
        let series = df
            .column(ticker.as_str())
            .with_context(|| format!("Column '{}' not found in {}", ticker, path.display()))?;

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

/// Load the 5 OHLCV fields from a parquet into [open, high, low, close, volume].
///
/// Supports both formats:
/// - **MultiIndex**: columns like `"('open', 'AAPL')"`
/// - **Wide**: columns like `"AAPL_open"` (underscore-separated)
fn load_ohlcv(
    path: &Path,
    canonical_tickers: &[String],
    format: &OhlcvFormat,
) -> Result<[WideMatrix; 5]> {
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
        let (field_idx, col_idx) = match format {
            OhlcvFormat::MultiIndex => {
                // Parse "('field', 'ticker')" format
                let trimmed =
                    col_name.trim_matches(|c: char| c == '(' || c == ')' || c == ' ');
                let parts: Vec<&str> = trimmed.split(',').collect();
                if parts.len() != 2 {
                    continue;
                }
                let field = parts[0].trim().trim_matches('\'');
                let ticker = parts[1].trim().trim_matches('\'');
                let fi = match fields.iter().position(|f| *f == field) {
                    Some(i) => i,
                    None => continue,
                };
                let ci = match ticker_idx.get(ticker) {
                    Some(&i) => i,
                    None => continue,
                };
                (fi, ci)
            }
            OhlcvFormat::Wide => {
                // Parse "TICKER_field" format
                let mut found = None;
                for (fi, &field) in fields.iter().enumerate() {
                    let suffix = format!("_{}", field);
                    if let Some(ticker) = col_name.strip_suffix(&suffix) {
                        if let Some(&ci) = ticker_idx.get(ticker) {
                            found = Some((fi, ci));
                            break;
                        }
                    }
                }
                match found {
                    Some(pair) => pair,
                    None => continue,
                }
            }
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

/// Extract 5m timestamps as millis-since-epoch from the index column.
fn extract_5m_timestamps(path: &Path, index_columns: &[String]) -> Result<Vec<i64>> {
    let df = LazyFrame::scan_parquet(path, Default::default())?.collect()?;

    let col_names: Vec<String> = df
        .get_column_names_str()
        .iter()
        .map(|s| s.to_string())
        .collect();

    let index_col = index_columns
        .iter()
        .find(|c| col_names.contains(c))
        .cloned()
        .or_else(|| {
            col_names
                .iter()
                .find(|c| c.starts_with("__index_level_") || c.as_str() == "timestamp")
                .cloned()
        })
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

        while cursor < n_5m && ts_5m[cursor] < day_start_ms {
            cursor += 1;
        }
        let start = cursor;

        let mut end = start;
        while end < n_5m && ts_5m[end] < day_end_ms {
            end += 1;
        }

        mapping[di] = (start, end);
    }

    mapping
}
