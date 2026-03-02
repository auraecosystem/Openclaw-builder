//! Data loading and storage crate: polars-based parquet I/O isolated here
//! so the rest of the workspace avoids the heavy polars dependency.

pub mod compute;
mod indicator;
mod loader;
pub mod resample;
mod store;

pub use indicator::Indicator;
pub use loader::{load_data_store, resolve_date_row};
pub use resample::resample_ohlcv;
pub use store::{Axes, DataStore, IntradayData};

// Re-export matrix types from engine-types for convenience (callers that
// depend on engine-data often also need WideMatrix/WideMask).
pub use engine_types::{WideMask, WideMatrix};
