# Engine Refactor Spec

## Current Source Files (read these directly)

- `src/types.rs` — WideMatrix, WideMask, Axes, DataStore (34-field god struct), Params, Trade, Direction, SignalSet, ResolvedParams
- `src/data.rs` — Parquet loading: load_wide_parquet, extract_dates, load_ohlcv, load_etf_set, extract_5m_timestamps, build_day_to_5m, load_data_store, resolve_date_row
- `src/simulate.rs` — 3 near-identical loops: find_5m_entry, check_5m_stop, simulate_breakout (quick+runner), simulate_single/simulate_ticker_single
- `src/filter.rs` — compute_regime, build_breakout_universe, build_ep_universe, build_parabolic_universe
- `src/signal.rs` — breakout_signals, ep_signals, parabolic_signals
- `src/batch.rs` — run_batch (rayon par_iter), run_single (match setup → filter → signal → simulate → report)
- `src/metrics.rs` — Pure math: sharpe, sortino, cagr, max_drawdown, profit_factor, equity_to_returns, estimate_annualization_from_dates
- `src/report.rs` — Report struct + generate_report
- `src/main.rs` — CLI dispatch + TCP server (serve_mode)
- `src/cli.rs` — Clap CLI struct + to_params()
- `Cargo.toml` — polars 0.46, rayon 1.10, clap 4, serde, chrono, anyhow, strum 0.26

## Key Refactor Goals

1. Replace 34-field DataStore god struct with `Indicator` enum + `Vec<WideMatrix>` indexed by `Indicator as usize`
2. Replace 7 `Option<>` 5m fields with structured `IntradayData`
3. Replace 3 copy-pasted simulation loops with one generic `simulate()` parameterized by ExitRule[], PositionSizer, FillMode, Direction
4. Extract 3x duplicated position sizing into `PositionSizer::compute()`
5. Move filter+signal logic into Setup trait implementations per strategy
6. Separate concerns into `data/`, `execution/`, `strategy/`, `analysis/`, `server/` modules

---

## Target API Signatures

### `data/indicator.rs`
```rust
use strum::{EnumCount, EnumIter};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, EnumCount, EnumIter)]
#[repr(u8)]
pub enum Indicator {
    Open, High, Low, Close, Volume,
    Atr14, Sma10, Sma20, VolSma20,
    RsPctrank1m, RsPctrank3m, RsPctrank6m,
    Dist52w, Ret63, Ret126, Pct10d, ConsecGreen,
    VcpNumContractions, VcpLastContractionPct, VcpTighteningRatio, VcpVolTrend,
    FlagPolePct, FlagRetracePct, FlagDays, FlagVolRatio,
    ConsolHigh,
}

impl Indicator {
    /// Returns the parquet cache filename for this indicator, or None for OHLCV (loaded separately).
    pub fn cache_filename(&self) -> Option<&'static str> {
        match self {
            Self::Open | Self::High | Self::Low | Self::Close | Self::Volume => None,
            Self::Atr14 => Some("atr_14.parquet"),
            Self::Sma10 => Some("sma_10.parquet"),
            Self::Sma20 => Some("sma_20.parquet"),
            Self::VolSma20 => Some("vol_sma_20.parquet"),
            Self::RsPctrank1m => Some("rs_pctrank_1m.parquet"),
            Self::RsPctrank3m => Some("rs_pctrank_3m.parquet"),
            Self::RsPctrank6m => Some("rs_pctrank_6m.parquet"),
            Self::Dist52w => Some("dist_52w.parquet"),
            Self::Ret63 => Some("ret_63.parquet"),
            Self::Ret126 => Some("ret_126.parquet"),
            Self::Pct10d => Some("pct_10d.parquet"),
            Self::ConsecGreen => Some("consec_green.parquet"),
            Self::VcpNumContractions => Some("vcp_num_contractions.parquet"),
            Self::VcpLastContractionPct => Some("vcp_last_contraction_pct.parquet"),
            Self::VcpTighteningRatio => Some("vcp_tightening_ratio.parquet"),
            Self::VcpVolTrend => Some("vcp_vol_trend.parquet"),
            Self::FlagPolePct => Some("flag_pole_pct.parquet"),
            Self::FlagRetracePct => Some("flag_retrace_pct.parquet"),
            Self::FlagDays => Some("flag_days.parquet"),
            Self::FlagVolRatio => Some("flag_vol_ratio.parquet"),
            Self::ConsolHigh => Some("consol_high.parquet"),
        }
    }

    pub fn is_ohlcv(&self) -> bool {
        matches!(self, Self::Open | Self::High | Self::Low | Self::Close | Self::Volume)
    }
}
```

### `data/matrix.rs`
```rust
/// Row-major f32 matrix: data[row * n_cols + col].
pub struct WideMatrix {
    pub(crate) data: Vec<f32>,
    n_rows: usize,
    n_cols: usize,
}

impl WideMatrix {
    pub fn new(data: Vec<f32>, n_rows: usize, n_cols: usize) -> Self {
        debug_assert_eq!(data.len(), n_rows * n_cols);
        Self { data, n_rows, n_cols }
    }

    #[inline(always)]
    pub fn get(&self, row: usize, col: usize) -> f32 {
        self.data[row * self.n_cols + col]
    }

    pub fn n_rows(&self) -> usize { self.n_rows }
    pub fn n_cols(&self) -> usize { self.n_cols }
    pub(crate) fn data_mut(&mut self) -> &mut [f32] { &mut self.data }
}

/// Boolean mask, same shape as WideMatrix.
pub struct WideMask {
    pub(crate) data: Vec<bool>,
    n_rows: usize,
    n_cols: usize,
}

impl WideMask {
    pub fn new_false(n_rows: usize, n_cols: usize) -> Self {
        Self { data: vec![false; n_rows * n_cols], n_rows, n_cols }
    }

    #[inline(always)]
    pub fn get(&self, row: usize, col: usize) -> bool {
        self.data[row * self.n_cols + col]
    }

    #[inline(always)]
    pub fn set(&mut self, row: usize, col: usize, val: bool) {
        self.data[row * self.n_cols + col] = val;
    }

    pub fn n_rows(&self) -> usize { self.n_rows }
    pub fn n_cols(&self) -> usize { self.n_cols }
}
```

### `data/store.rs`
```rust
use std::collections::HashMap;
use super::indicator::Indicator;
use super::matrix::WideMatrix;

pub struct Axes {
    pub dates: Vec<i32>,
    pub tickers: Vec<String>,
    pub ticker_idx: HashMap<String, usize>,
    pub spy_col: Option<usize>,
    pub etf_cols: Vec<bool>,
    pub n_rows: usize,
    pub n_cols: usize,
}

pub struct IntradayData {
    pub matrices: Vec<WideMatrix>,  // indexed by Indicator as usize (OHLCV only)
    pub timestamps: Vec<i64>,
    pub day_mapping: Vec<(usize, usize)>,  // daily_row → (start_5m, end_5m)
}

pub struct DataStore {
    pub axes: Axes,
    daily: Vec<WideMatrix>,  // indexed by Indicator as usize, length = Indicator::COUNT
    pub intraday: Option<IntradayData>,
}

impl DataStore {
    pub fn new(axes: Axes, daily: Vec<WideMatrix>, intraday: Option<IntradayData>) -> Self {
        assert_eq!(daily.len(), Indicator::COUNT);
        Self { axes, daily, intraday }
    }

    #[inline(always)]
    pub fn get(&self, ind: Indicator) -> &WideMatrix {
        &self.daily[ind as usize]
    }

    #[inline(always)] pub fn open(&self) -> &WideMatrix { self.get(Indicator::Open) }
    #[inline(always)] pub fn high(&self) -> &WideMatrix { self.get(Indicator::High) }
    #[inline(always)] pub fn low(&self) -> &WideMatrix { self.get(Indicator::Low) }
    #[inline(always)] pub fn close(&self) -> &WideMatrix { self.get(Indicator::Close) }
    #[inline(always)] pub fn volume(&self) -> &WideMatrix { self.get(Indicator::Volume) }
}
```

### `data/loader.rs`
```rust
use std::path::Path;
use anyhow::Result;
use super::store::{Axes, DataStore};

/// Load all data into a DataStore. Refactored from current data.rs.
/// Uses Indicator::iter() to load cache files in parallel via rayon.
pub fn load_data_store(data_dir: &Path) -> Result<DataStore>;

/// Resolve date strings ("2020-01-01") to row indices.
pub fn resolve_date_row(axes: &Axes, date_str: &str) -> Option<usize>;
```

### `data/mod.rs`
```rust
mod indicator;
mod loader;
mod matrix;
mod store;

pub use indicator::Indicator;
pub use loader::{load_data_store, resolve_date_row};
pub use matrix::{WideMask, WideMatrix};
pub use store::{Axes, DataStore, IntradayData};
```

### `types.rs` (trimmed — remove WideMatrix/WideMask/Axes/DataStore, keep the rest)
```rust
// Keep: Params (all serde defaults), Trade, Direction, SignalSet, ResolvedParams
// SignalSet now uses data::WideMask and data::WideMatrix
use crate::data::{WideMask, WideMatrix};

pub struct SignalSet {
    pub entries: WideMask,
    pub exits: WideMask,
    pub stop_prices: WideMatrix,
}

pub struct ResolvedParams {
    pub params: Params,
    pub start_row: usize,
    pub end_row: usize,
}
// Params, Trade, Direction — unchanged from current types.rs
```

### `execution/exits.rs`
```rust
use crate::data::{DataStore, Indicator};
use crate::types::Direction;

pub struct ExitContext {
    pub row: usize,
    pub col: usize,
    pub close: f32,
    pub entry_price: f32,
    pub bars_since_entry: usize,
    pub active_stop: f32,
    pub direction: Direction,
}

#[derive(Clone, Debug)]
pub enum ExitRule {
    /// Exit when close < SMA. Used by breakout quick (SMA10) and runner (SMA10).
    SmaCross { sma: Indicator },
    /// Exit after min_bars if profitable. Used by breakout quick (5 bars).
    ProfitTarget { min_bars: usize },
    /// Upgrade stop to entry price after N bars if profitable. Used by breakout runner.
    BreakevenUpgrade { after_bars: usize },
    /// Exit on stop loss hit (close <= active_stop for long, >= for short).
    StopLoss,
    /// Cover short when close <= either SMA. Used by parabolic.
    SmaCover { sma1: Indicator, sma2: Indicator },
}

impl ExitRule {
    /// Returns true if this rule triggers an exit.
    #[inline]
    pub fn should_exit(&self, ctx: &ExitContext, store: &DataStore) -> bool {
        // Implementation based on current simulate.rs exit logic
    }

    /// Returns Some(new_stop) if this rule wants to update the trailing stop.
    #[inline]
    pub fn update_stop(&self, ctx: &ExitContext) -> Option<f32> {
        // BreakevenUpgrade returns Some(entry_price) when conditions met
    }
}
```

### `execution/position.rs`
```rust
use crate::types::{Direction, Params};

pub struct PositionSizer {
    pub risk_pct: f32,
    pub max_pos_pct: f32,
    pub slippage_k: f32,
}

impl PositionSizer {
    pub fn from_params(params: &Params) -> Self {
        Self { risk_pct: params.risk_pct, max_pos_pct: params.max_pos_pct, slippage_k: params.slippage_k }
    }

    /// Compute (shares, slippage-adjusted fill price).
    /// Extracts the 10-line position sizing block duplicated 3x in current simulate.rs.
    #[inline]
    pub fn compute(
        &self, equity: f64, fill_price: f32, stop: f32,
        adv: f32, close: f32, direction: Direction,
    ) -> (f32, f32) {
        // risk_shares, max_shares, liquidity_cap, slippage — from current simulate.rs
    }
}
```

### `execution/fills.rs`
```rust
use crate::data::DataStore;
use crate::types::Direction;

#[derive(Clone, Copy, Debug)]
pub enum FillMode {
    /// Fill at next day's open (breakout, parabolic).
    NextDayOpen,
    /// Signal already shifted to entry day; fill at that day's open (EP).
    SameDayOpen,
    /// Daily signal + 5m confirmation (dual-timeframe breakout).
    DualTimeframe,
}

/// Resolve entry fill price and actual entry row.
/// Returns None if fill is invalid (NaN, out of range, no 5m confirmation).
pub fn resolve_fill(
    store: &DataStore, signal_row: usize, col: usize,
    mode: FillMode, end_row: usize,
) -> Option<(f32, usize)>;

/// Check stop on 5m bars for the current daily row.
/// Returns Some(exit_price) if stop was hit intraday.
pub fn check_5m_stop(
    store: &DataStore, daily_row: usize, col: usize,
    stop_price: f32, direction: Direction,
) -> Option<f32>;
```

### `execution/simulator.rs`
```rust
use crate::data::DataStore;
use crate::types::{Direction, ResolvedParams, SignalSet, Trade};
use super::exits::ExitRule;
use super::fills::FillMode;
use super::position::PositionSizer;

/// Generic simulation loop. Replaces simulate_breakout_quick, simulate_breakout_runner,
/// and simulate_ticker_single with ONE parameterized function.
/// Called per-ticker (col), returns trades for that ticker.
pub fn simulate(
    store: &DataStore,
    signals: &SignalSet,
    rp: &ResolvedParams,
    sizer: &PositionSizer,
    exit_rules: &[ExitRule],
    direction: Direction,
    equity_fraction: f32,
    fill_mode: FillMode,
    setup_name: &'static str,
) -> Vec<Trade>;
```

### `execution/mod.rs`
```rust
mod exits;
mod fills;
mod position;
mod simulator;

pub use exits::{ExitContext, ExitRule};
pub use fills::{check_5m_stop, resolve_fill, FillMode};
pub use position::PositionSizer;
pub use simulator::simulate;
```

### `strategy/mod.rs`
```rust
use std::ops::Range;
use crate::data::{DataStore, WideMask};
use crate::execution::{ExitRule, FillMode};
use crate::types::{Direction, Params, SignalSet};

pub trait Setup: Send + Sync {
    fn name(&self) -> &'static str;
    fn direction(&self) -> Direction;
    fn equity_fraction(&self) -> f32 { 1.0 }
    fn fill_mode(&self) -> FillMode { FillMode::NextDayOpen }
    fn exit_rules(&self) -> Vec<ExitRule>;
    /// Build universe filter mask for the given date range.
    fn filter(&self, store: &DataStore, params: &Params, range: Range<usize>) -> WideMask;
    /// Generate entry/exit/stop signals from the filtered universe.
    fn signals(&self, store: &DataStore, universe: &WideMask, params: &Params) -> SignalSet;
}

/// Factory: "breakout" → [BreakoutQuick, BreakoutRunner], "ep" → [EpisodicPivot], "parabolic" → [ParabolicShort]
pub fn create_setups(name: &str) -> Vec<Box<dyn Setup>>;
```

### `strategy/breakout.rs`
```rust
// Two structs: BreakoutQuick and BreakoutRunner
// Both share filter logic (from current build_breakout_universe) and signal logic (from breakout_signals)
// Differ in: exit_rules, equity_fraction, name
// BreakoutQuick: equity_fraction = split_frac, exits = [ProfitTarget{5}, SmaCross{Sma10}, StopLoss]
// BreakoutRunner: equity_fraction = 1-split_frac, exits = [BreakevenUpgrade{5}, SmaCross{Sma10}, StopLoss]
```

### `strategy/ep.rs`
```rust
// EpisodicPivot struct
// filter = build_ep_universe logic (from current filter.rs)
// signals = ep_signals logic (from current signal.rs)
// fill_mode = FillMode::SameDayOpen
// exit_rules = [SmaCross{Sma10}, StopLoss]
```

### `strategy/parabolic.rs`
```rust
// ParabolicShort struct
// direction = Direction::Short
// filter = build_parabolic_universe logic (from current filter.rs)
// signals = parabolic_signals logic (from current signal.rs)
// exit_rules = [SmaCover{Sma10, Sma20}, StopLoss]
```

### `analysis/metrics.rs`
Copy verbatim from current `src/metrics.rs`. No changes.

### `analysis/report.rs`
```rust
// Same Report struct and generate_report function from current report.rs
// Only change: imports from crate::data::Axes instead of crate::types::Axes
```

### `analysis/mod.rs`
```rust
pub mod metrics;
pub mod report;
pub use report::{generate_report, Report};
```

### `server/tcp.rs`
```rust
use crate::data::DataStore;
use anyhow::Result;

/// TCP server extracted from current main.rs serve_mode().
pub fn serve(store: &DataStore, port: u16) -> Result<()>;
```

### `server/mod.rs`
```rust
mod tcp;
pub use tcp::serve;
```

### `lib.rs`
```rust
pub mod data;
pub mod execution;
pub mod strategy;
pub mod analysis;
pub mod server;
pub mod types;

use std::path::Path;
use anyhow::Result;
use data::DataStore;
use analysis::Report;

pub fn run_single(store: &DataStore, params: &types::Params) -> Report;
pub fn run_batch(store: &DataStore, params_file: &Path) -> Result<Vec<Report>>;
```

### `main.rs` (slim CLI dispatch only)
```rust
mod cli;
// No more mod declarations — everything comes from lib.rs
use swingtrader_engine::{data, server, run_single, run_batch};

fn main() -> Result<()> {
    // Same CLI dispatch as current main.rs but calling lib functions
}
```

---

## Task Assignments

### Agent A: `data/` module (5 files)

Write: `src/data/indicator.rs`, `src/data/matrix.rs`, `src/data/store.rs`, `src/data/loader.rs`, `src/data/mod.rs`

Read current: `src/types.rs` (WideMatrix/WideMask/Axes/DataStore), `src/data.rs` (all loading logic)

Key changes:
- WideMatrix/WideMask: fields become private, add `new()` constructor, `n_rows()`/`n_cols()` accessors, `pub(crate) data` field
- Indicator enum: 26 variants with `#[repr(u8)]`, strum derives, `cache_filename()` mapping, `is_ohlcv()` helper
- DataStore: `daily: Vec<WideMatrix>` with length `Indicator::COUNT`, indexed by `ind as usize`. Convenience `open()/high()/low()/close()/volume()` methods.
- IntradayData: replaces the 7 `Option<>` fields (open_5m, high_5m, etc + dates_5m + day_to_5m)
- loader.rs: refactor `load_data_store()` to use `Indicator::iter()` for parallel cache loading. Move `resolve_date_row()` here. Keep all parquet loading helpers (load_wide_parquet, extract_dates, load_ohlcv, load_etf_set, extract_5m_timestamps, build_day_to_5m) as private functions in loader.rs.

### Agent B: `execution/` module (5 files)

Write: `src/execution/exits.rs`, `src/execution/position.rs`, `src/execution/fills.rs`, `src/execution/simulator.rs`, `src/execution/mod.rs`

Read current: `src/simulate.rs` (all 3 loops + find_5m_entry + check_5m_stop)

Key changes:
- exits.rs: ExitRule enum with 5 variants + ExitContext struct. `should_exit()` extracts exit conditions from the 3 loops. `update_stop()` handles BreakevenUpgrade.
- position.rs: Extract the position sizing block (risk_shares, max_shares, liquidity_cap, slippage, adjusted_fill) into `PositionSizer::compute()`. This block appears 3x identically in current simulate.rs.
- fills.rs: Extract `find_5m_entry()` as `resolve_fill()` with FillMode::DualTimeframe. `check_5m_stop()` moves here. FillMode::NextDayOpen and SameDayOpen handle the other two fill patterns.
- simulator.rs: One generic `simulate()` that: finds active cols, par_iter over tickers, calls `resolve_fill()`, calls `sizer.compute()`, loops checking `exit_rule.should_exit()` and `exit_rule.update_stop()`. The outer caller (strategy via lib.rs) provides the ExitRule list, PositionSizer, FillMode, Direction.

### Agent C: `strategy/` module (4 files)

Write: `src/strategy/mod.rs`, `src/strategy/breakout.rs`, `src/strategy/ep.rs`, `src/strategy/parabolic.rs`

Read current: `src/filter.rs`, `src/signal.rs`

Key changes:
- mod.rs: Setup trait + `create_setups()` factory
- breakout.rs: Two structs `BreakoutQuick` and `BreakoutRunner`, both implementing Setup. Share `filter()` (= current `build_breakout_universe`) and `signals()` (= current `breakout_signals`). Differ in `exit_rules()` and `equity_fraction()`. `compute_regime()` is a private helper here.
- ep.rs: `EpisodicPivot` struct. `filter()` = current `build_ep_universe`. `signals()` = current `ep_signals`. `fill_mode()` = `SameDayOpen`.
- parabolic.rs: `ParabolicShort` struct. `direction()` = `Short`. `filter()` = current `build_parabolic_universe`. `signals()` = current `parabolic_signals`.

### Agent D: glue files (7 files)

Write: `src/types.rs` (trimmed), `src/lib.rs`, `src/main.rs` (rewrite), `src/analysis/mod.rs`, `src/analysis/metrics.rs`, `src/analysis/report.rs`, `src/server/mod.rs`, `src/server/tcp.rs`

Read current: `src/types.rs`, `src/batch.rs`, `src/main.rs`, `src/metrics.rs`, `src/report.rs`, `src/cli.rs`

Key changes:
- types.rs: Remove WideMatrix, WideMask, Axes, DataStore (now in data module). Keep Params (all serde defaults + Default impl), Trade, Direction, SignalSet (using `crate::data::{WideMask, WideMatrix}`), ResolvedParams.
- lib.rs: Crate root. Declares modules: data, execution, strategy, analysis, server, types. Exports `run_single()` (replaces batch::run_single, uses `strategy::create_setups()`) and `run_batch()` (rayon par_iter).
- main.rs: Slim CLI only. `mod cli;` + `use swingtrader_engine::*;`. Load data, dispatch to serve/batch/single.
- analysis/metrics.rs: Verbatim copy of current metrics.rs
- analysis/report.rs: Current report.rs with imports changed from `crate::types::Axes` to `crate::data::Axes`
- analysis/mod.rs: Re-export metrics + report
- server/tcp.rs: Extract serve_mode() from current main.rs
- server/mod.rs: Re-export serve

### Agent E: tests (3 files)

Write: `tests/metrics_test.rs`, `tests/position_test.rs`, `tests/exits_test.rs`

Key changes:
- metrics_test.rs: Unit tests for sharpe, sortino, cagr, max_drawdown, profit_factor, equity_to_returns — known inputs/outputs
- position_test.rs: Unit tests for PositionSizer::compute() — verify risk sizing, liquidity cap, slippage
- exits_test.rs: Unit tests for each ExitRule variant's should_exit() and update_stop()
