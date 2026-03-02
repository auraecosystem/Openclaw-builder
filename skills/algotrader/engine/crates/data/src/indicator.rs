use strum::{EnumCount, EnumIter};

/// Typed indicator index. Each variant maps 1:1 to a `WideMatrix` slot in
/// `DataStore::daily`.  The discriminant order is fixed (`#[repr(u8)]`) so
/// `ind as usize` is a stable index into the `Vec<WideMatrix>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, EnumCount, EnumIter)]
#[repr(u8)]
pub enum Indicator {
    // --- OHLCV (loaded from MultiIndex parquet via load_ohlcv) ---
    Open,
    High,
    Low,
    Close,
    Volume,
    // --- Technical indicators (each from its own cache parquet) ---
    Atr14,
    Sma10,
    Sma20,
    VolSma20,
    RsPctrank1m,
    RsPctrank3m,
    RsPctrank6m,
    Dist52w,
    Ret63,
    Ret126,
    Pct10d,
    ConsecGreen,
    // --- VCP intermediates ---
    VcpNumContractions,
    VcpLastContractionPct,
    VcpTighteningRatio,
    VcpVolTrend,
    // --- Flag intermediates ---
    FlagPolePct,
    FlagRetracePct,
    FlagDays,
    FlagVolRatio,
    // --- Other ---
    ConsolHigh,
}

impl Indicator {
    /// Returns the parquet cache filename for this indicator, or `None` for
    /// OHLCV fields and runtime-computed indicators (no cache file needed).
    pub fn cache_filename(&self) -> Option<&'static str> {
        match self {
            // OHLCV: loaded from combined parquet
            Self::Open | Self::High | Self::Low | Self::Close | Self::Volume => None,
            // Runtime-computed: derived from OHLCV at load time (no cache)
            Self::Atr14 => None,
            Self::Sma10 => None,
            Self::Sma20 => None,
            Self::VolSma20 => None,
            Self::Dist52w => None,
            Self::Ret63 => None,
            Self::Ret126 => None,
            Self::Pct10d => None,
            Self::ConsolHigh => None,
            Self::ConsecGreen => None,
            // Cross-sectional: must stay cached (needs all tickers simultaneously)
            Self::RsPctrank1m => Some("rs_pctrank_1m.parquet"),
            Self::RsPctrank3m => Some("rs_pctrank_3m.parquet"),
            Self::RsPctrank6m => Some("rs_pctrank_6m.parquet"),
            // Multi-pass pattern detection: stays cached
            Self::VcpNumContractions => Some("vcp_num_contractions.parquet"),
            Self::VcpLastContractionPct => Some("vcp_last_contraction_pct.parquet"),
            Self::VcpTighteningRatio => Some("vcp_tightening_ratio.parquet"),
            Self::VcpVolTrend => Some("vcp_vol_trend.parquet"),
            Self::FlagPolePct => Some("flag_pole_pct.parquet"),
            Self::FlagRetracePct => Some("flag_retrace_pct.parquet"),
            Self::FlagDays => Some("flag_days.parquet"),
            Self::FlagVolRatio => Some("flag_vol_ratio.parquet"),
        }
    }

    /// True for OHLCV fields (loaded from the combined OHLCV parquet).
    pub fn is_ohlcv(&self) -> bool {
        matches!(
            self,
            Self::Open | Self::High | Self::Low | Self::Close | Self::Volume
        )
    }

    /// True for indicators computed at runtime from OHLCV (no cache file needed).
    /// These 10 are single-pass, column-local, SIMD-friendly operations.
    pub fn is_runtime_computed(&self) -> bool {
        matches!(
            self,
            Self::Atr14
                | Self::Sma10
                | Self::Sma20
                | Self::VolSma20
                | Self::Dist52w
                | Self::Ret63
                | Self::Ret126
                | Self::Pct10d
                | Self::ConsolHigh
                | Self::ConsecGreen
        )
    }
}
