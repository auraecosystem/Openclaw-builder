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
    /// OHLCV fields (those are loaded separately from the combined parquet).
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

    /// True for Open/High/Low/Close/Volume -- loaded from the combined OHLCV
    /// parquet rather than individual cache files.
    pub fn is_ohlcv(&self) -> bool {
        matches!(
            self,
            Self::Open | Self::High | Self::Low | Self::Close | Self::Volume
        )
    }
}
