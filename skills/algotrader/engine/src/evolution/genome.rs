//! Genome encoding/decoding: maps flat Vec<f64> to/from Params + SignalParams.
//!
//! Each gene is a single evolvable scalar field. The `GenomeSpec` defines
//! ordered bounds, encode/decode routines, and clamping. Genome vectors are
//! plain `Vec<f64>` so they work with any optimizer (CMA-ES, DE, GA).

use engine_types::{Params, PatternParams, SignalParams, PATTERN_FEATURE_COUNT, SCANNER_FEATURE_COUNT};

// ---------------------------------------------------------------------------
// Gene bound metadata
// ---------------------------------------------------------------------------

/// Metadata for a single gene (one evolvable field).
#[derive(Clone, Debug)]
pub struct GeneBound {
    pub name: &'static str,
    pub min: f64,
    pub max: f64,
    pub is_integer: bool,
    pub is_boolean: bool,
}

/// Specification of the full genome: ordered list of gene bounds.
#[derive(Clone, Debug)]
pub struct GenomeSpec {
    pub bounds: Vec<GeneBound>,
}

// ---------------------------------------------------------------------------
// Constructors
// ---------------------------------------------------------------------------

impl GenomeSpec {
    /// Build a spec for the 14 evolvable `Params` fields.
    ///
    /// When `crypto` is true, several ranges widen to suit the noisier
    /// crypto universe (fewer tickers, different volume/volatility profile).
    pub fn params_spec(crypto: bool) -> Self {
        let bounds = if crypto {
            vec![
                gene("rs_pct", 0.20, 0.60),
                gene("vol_ratio", 1.2, 3.0),
                gene("max_range_pct", 0.15, 0.50),
                gene("max_dist_52w", 0.10, 0.50),
                gene("min_adv", 0.0, 5e5),
                gene("slippage_k", 0.05, 0.20),
                gene("min_prior_move", 0.0, 0.30),
                gene("max_sma_ext", 0.05, 0.30),
                gene("risk_pct", 0.003, 0.010),
                gene("max_pos_pct", 0.10, 0.30),
                gene("split_frac", 0.20, 0.80),
                gene("min_adr_pct", 0.0, 0.05),
                igene("min_consol_days", 0.0, 10.0),
                bgene("regime"),
            ]
        } else {
            vec![
                gene("rs_pct", 0.01, 0.20),
                gene("vol_ratio", 1.2, 3.0),
                gene("max_range_pct", 0.10, 0.30),
                gene("max_dist_52w", 0.10, 0.50),
                gene("min_adv", 5e6, 3e8),
                gene("slippage_k", 0.05, 0.20),
                gene("min_prior_move", 0.0, 0.50),
                gene("max_sma_ext", 0.05, 0.30),
                gene("risk_pct", 0.003, 0.010),
                gene("max_pos_pct", 0.10, 0.30),
                gene("split_frac", 0.05, 0.95),
                gene("min_adr_pct", 0.02, 0.20),
                igene("min_consol_days", 3.0, 20.0),
                bgene("regime"),
            ]
        };
        Self { bounds }
    }

    /// Build a spec for the ~30 evolvable `SignalParams` fields.
    pub fn signal_params_spec() -> Self {
        let mut bounds = vec![
            igene("window_len", 50.0, 500.0),
            gene("candidate_threshold", 0.1, 0.9),
            igene("l0_recompute_interval", 20.0, 500.0),
            igene("pe_order", 3.0, 7.0),
            igene("pe_delay", 1.0, 5.0),
            igene("pe_window", 10.0, 100.0),
            gene("bocpd_lambda", 10.0, 500.0),
            gene("kalman_q", 0.0001, 0.1),
            gene("kalman_r", 0.01, 10.0),
            igene("vmd_k_modes", 2.0, 6.0),
            gene("vmd_alpha", 100.0, 5000.0),
            igene("vmd_sta_window", 3.0, 30.0),
            igene("vmd_lta_window", 20.0, 200.0),
            igene("knn_k", 2.0, 20.0),
            igene("scatter_j", 3.0, 10.0),
            igene("stomp_m", 10.0, 100.0),
            igene("template_len", 20.0, 200.0),
            igene("swt_level", 1.0, 6.0),
            igene("hurst_window", 100.0, 1000.0),
            igene("hmm_n_states", 2.0, 5.0),
            igene("rmt_window", 100.0, 1000.0),
            gene("conformal_coverage", 0.5, 0.99),
            igene("conformal_window", 50.0, 500.0),
            igene("vpin_n_buckets", 10.0, 200.0),
            gene("scorer_bias", -2.0, 2.0),
            gene("kalman_stop_atr_mult", 0.5, 5.0),
            gene("bocpd_exit_threshold", 0.3, 0.95),
            gene("vmd_entry_threshold", 0.5, 5.0),
        ];

        // 13 scorer weights, one per scanner feature.
        for i in 0..SCANNER_FEATURE_COUNT {
            bounds.push(gene(scorer_weight_name(i), 0.0, 3.0));
        }

        Self { bounds }
    }

    /// Build a spec for the ~19 evolvable `PatternParams` fields (bull flag tuning).
    pub fn pattern_params_spec() -> Self {
        let mut bounds = vec![
            // Bull flag components
            gene("flag_pole_mid", 0.02, 0.15),
            gene("flag_pole_k", 5.0, 50.0),
            igene("flag_pole_min_bars", 2.0, 15.0),
            gene("flag_slope_center", -0.5, 0.1),
            gene("flag_slope_sigma", 0.05, 1.0),
            gene("flag_retrace_lo", 0.10, 0.50),
            gene("flag_retrace_hi", 0.30, 0.80),
            gene("flag_vol_mid", 0.3, 1.2),
            gene("flag_vol_k", -30.0, -1.0),
            igene("flag_max_bars", 10.0, 60.0),
            igene("flag_lookback", 20.0, 120.0),
            // Composite scoring
            gene("pattern_scorer_bias", -2.0, 2.0),
            gene("pattern_alpha", 0.0, 1.0),
        ];

        // 4 bull flag component weights: [pole, slope, retracement, volume]
        for i in 0..4 {
            bounds.push(gene(flag_weight_name(i), 0.0, 1.0));
        }

        // 4 multi-timeframe scorer weights: [daily, 1h, 30m, 5m]
        for i in 0..PATTERN_FEATURE_COUNT {
            bounds.push(gene(pattern_scorer_weight_name(i), 0.0, 3.0));
        }

        Self { bounds }
    }

    /// Concatenation of params_spec + signal_params_spec for joint optimization.
    pub fn combined_spec(crypto: bool) -> Self {
        let mut combined = Self::params_spec(crypto);
        combined.bounds.extend(Self::signal_params_spec().bounds);
        combined
    }

    /// Concatenation of params_spec + signal_params_spec + pattern_params_spec.
    pub fn combined_with_patterns_spec(crypto: bool) -> Self {
        let mut combined = Self::params_spec(crypto);
        combined.bounds.extend(Self::signal_params_spec().bounds);
        combined.bounds.extend(Self::pattern_params_spec().bounds);
        combined
    }

    /// params_spec + pattern_params_spec (no signals).
    pub fn params_with_patterns_spec(crypto: bool) -> Self {
        let mut combined = Self::params_spec(crypto);
        combined.bounds.extend(Self::pattern_params_spec().bounds);
        combined
    }
}

// ---------------------------------------------------------------------------
// Encode / Decode / Clamp / Random
// ---------------------------------------------------------------------------

impl GenomeSpec {
    /// Number of genes in this spec.
    pub fn dim(&self) -> usize {
        self.bounds.len()
    }

    /// Encode a `Params` (+ optional `SignalParams`) into a genome vector.
    ///
    /// Genes are extracted by name from the params struct. Fields not present
    /// in the spec are simply not encoded.
    pub fn encode(&self, params: &Params) -> Vec<f64> {
        self.bounds
            .iter()
            .map(|b| encode_field(b.name, params))
            .collect()
    }

    /// Decode a genome vector into a `Params`, starting from `base` for
    /// any non-evolved fields.
    ///
    /// Integer genes are rounded, boolean genes are thresholded at 0.5.
    pub fn decode(&self, genome: &[f64], base: &Params) -> Params {
        let mut p = base.clone();
        for (i, b) in self.bounds.iter().enumerate() {
            let v = genome[i];
            decode_field(b, v, &mut p);
        }
        p
    }

    /// Clamp each gene to its bounds, round integers, threshold booleans.
    pub fn clamp(&self, genome: &mut [f64]) {
        for (i, b) in self.bounds.iter().enumerate() {
            genome[i] = genome[i].clamp(b.min, b.max);
            if b.is_integer {
                genome[i] = genome[i].round();
            }
            if b.is_boolean {
                genome[i] = if genome[i] >= 0.5 { 1.0 } else { 0.0 };
            }
        }
    }

    /// Generate a random genome uniformly sampled within bounds.
    pub fn random(&self, rng: &mut impl rand::Rng) -> Vec<f64> {
        let mut genome: Vec<f64> = self
            .bounds
            .iter()
            .map(|b| rng.gen_range(b.min..=b.max))
            .collect();
        self.clamp(&mut genome);
        genome
    }
}

// ---------------------------------------------------------------------------
// Internal helpers: bound constructors
// ---------------------------------------------------------------------------

/// Continuous gene.
fn gene(name: &'static str, min: f64, max: f64) -> GeneBound {
    GeneBound {
        name,
        min,
        max,
        is_integer: false,
        is_boolean: false,
    }
}

/// Integer gene (continuous range, rounded on decode/clamp).
fn igene(name: &'static str, min: f64, max: f64) -> GeneBound {
    GeneBound {
        name,
        min,
        max,
        is_integer: true,
        is_boolean: false,
    }
}

/// Boolean gene: [0, 1] range, thresholded at 0.5.
fn bgene(name: &'static str) -> GeneBound {
    GeneBound {
        name,
        min: 0.0,
        max: 1.0,
        is_integer: false,
        is_boolean: true,
    }
}

// Scorer weight names are generated at compile time via a static array so
// match arms can use `&'static str` comparisons without allocation.
static SCORER_WEIGHT_NAMES: [&str; SCANNER_FEATURE_COUNT] = [
    "scorer_w0",
    "scorer_w1",
    "scorer_w2",
    "scorer_w3",
    "scorer_w4",
    "scorer_w5",
    "scorer_w6",
    "scorer_w7",
    "scorer_w8",
    "scorer_w9",
    "scorer_w10",
    "scorer_w11",
    "scorer_w12",
];

fn scorer_weight_name(i: usize) -> &'static str {
    SCORER_WEIGHT_NAMES[i]
}

static FLAG_WEIGHT_NAMES: [&str; 4] = [
    "flag_w0", "flag_w1", "flag_w2", "flag_w3",
];

fn flag_weight_name(i: usize) -> &'static str {
    FLAG_WEIGHT_NAMES[i]
}

static PATTERN_SCORER_WEIGHT_NAMES: [&str; PATTERN_FEATURE_COUNT] = [
    "pat_scorer_w0", "pat_scorer_w1", "pat_scorer_w2", "pat_scorer_w3",
];

fn pattern_scorer_weight_name(i: usize) -> &'static str {
    PATTERN_SCORER_WEIGHT_NAMES[i]
}

// ---------------------------------------------------------------------------
// Field-level encode: Params/SignalParams -> f64
// ---------------------------------------------------------------------------

fn encode_field(name: &str, params: &Params) -> f64 {
    match name {
        // -- Params fields --
        "rs_pct" => params.rs_pct as f64,
        "vol_ratio" => params.vol_ratio as f64,
        "max_range_pct" => params.max_range_pct as f64,
        "max_dist_52w" => params.max_dist_52w as f64,
        "min_adv" => params.min_adv as f64,
        "slippage_k" => params.slippage_k as f64,
        "min_prior_move" => params.min_prior_move as f64,
        "max_sma_ext" => params.max_sma_ext as f64,
        "risk_pct" => params.risk_pct as f64,
        "max_pos_pct" => params.max_pos_pct as f64,
        "split_frac" => params.split_frac as f64,
        "min_adr_pct" => params.min_adr_pct as f64,
        "min_consol_days" => params.min_consol_days as f64,
        "regime" => if params.regime { 1.0 } else { 0.0 },

        // -- PatternParams fields --
        n if is_pattern_gene(n) => encode_pattern_field(name, params.pattern_params.as_ref()),

        // -- SignalParams fields (default to SignalParams::default() if absent) --
        _ => encode_signal_field(name, params.signal_params.as_ref()),
    }
}

fn encode_signal_field(name: &str, sp: Option<&SignalParams>) -> f64 {
    let defaults = SignalParams::default();
    let sp = sp.unwrap_or(&defaults);

    match name {
        "window_len" => sp.window_len as f64,
        "candidate_threshold" => sp.candidate_threshold as f64,
        "l0_recompute_interval" => sp.l0_recompute_interval as f64,
        "pe_order" => sp.pe_order as f64,
        "pe_delay" => sp.pe_delay as f64,
        "pe_window" => sp.pe_window as f64,
        "bocpd_lambda" => sp.bocpd_lambda as f64,
        "kalman_q" => sp.kalman_q as f64,
        "kalman_r" => sp.kalman_r as f64,
        "vmd_k_modes" => sp.vmd_k_modes as f64,
        "vmd_alpha" => sp.vmd_alpha as f64,
        "vmd_sta_window" => sp.vmd_sta_window as f64,
        "vmd_lta_window" => sp.vmd_lta_window as f64,
        "knn_k" => sp.knn_k as f64,
        "scatter_j" => sp.scatter_j as f64,
        "stomp_m" => sp.stomp_m as f64,
        "template_len" => sp.template_len as f64,
        "swt_level" => sp.swt_level as f64,
        "hurst_window" => sp.hurst_window as f64,
        "hmm_n_states" => sp.hmm_n_states as f64,
        "rmt_window" => sp.rmt_window as f64,
        "conformal_coverage" => sp.conformal_coverage as f64,
        "conformal_window" => sp.conformal_window as f64,
        "vpin_n_buckets" => sp.vpin_n_buckets as f64,
        "scorer_bias" => sp.scorer_bias as f64,
        "kalman_stop_atr_mult" => sp.kalman_stop_atr_mult as f64,
        "bocpd_exit_threshold" => sp.bocpd_exit_threshold as f64,
        "vmd_entry_threshold" => sp.vmd_entry_threshold as f64,

        // scorer_w0..scorer_w12
        n if n.starts_with("scorer_w") => {
            let idx: usize = n["scorer_w".len()..].parse().unwrap_or(0);
            sp.scorer_weights[idx] as f64
        }

        unknown => panic!("encode_signal_field: unknown gene name '{unknown}'"),
    }
}

// ---------------------------------------------------------------------------
// Field-level encode/decode: PatternParams
// ---------------------------------------------------------------------------

/// Check whether a gene name belongs to the pattern params group.
fn is_pattern_gene(name: &str) -> bool {
    matches!(
        name,
        "flag_pole_mid"
            | "flag_pole_k"
            | "flag_pole_min_bars"
            | "flag_slope_center"
            | "flag_slope_sigma"
            | "flag_retrace_lo"
            | "flag_retrace_hi"
            | "flag_vol_mid"
            | "flag_vol_k"
            | "flag_max_bars"
            | "flag_lookback"
            | "pattern_scorer_bias"
            | "pattern_alpha"
    ) || name.starts_with("flag_w")
        || name.starts_with("pat_scorer_w")
}

fn encode_pattern_field(name: &str, pp: Option<&PatternParams>) -> f64 {
    let defaults = PatternParams::default();
    let pp = pp.unwrap_or(&defaults);

    match name {
        "flag_pole_mid" => pp.flag_pole_mid as f64,
        "flag_pole_k" => pp.flag_pole_k as f64,
        "flag_pole_min_bars" => pp.flag_pole_min_bars as f64,
        "flag_slope_center" => pp.flag_slope_center as f64,
        "flag_slope_sigma" => pp.flag_slope_sigma as f64,
        "flag_retrace_lo" => pp.flag_retrace_lo as f64,
        "flag_retrace_hi" => pp.flag_retrace_hi as f64,
        "flag_vol_mid" => pp.flag_vol_mid as f64,
        "flag_vol_k" => pp.flag_vol_k as f64,
        "flag_max_bars" => pp.flag_max_bars as f64,
        "flag_lookback" => pp.flag_lookback as f64,
        "pattern_scorer_bias" => pp.pattern_scorer_bias as f64,
        "pattern_alpha" => pp.pattern_alpha as f64,

        // flag_w0..flag_w3 (bull flag component weights)
        n if n.starts_with("flag_w") => {
            let idx: usize = n["flag_w".len()..].parse().unwrap_or(0);
            pp.flag_weights[idx] as f64
        }

        // pat_scorer_w0..pat_scorer_w3 (multi-timeframe weights)
        n if n.starts_with("pat_scorer_w") => {
            let idx: usize = n["pat_scorer_w".len()..].parse().unwrap_or(0);
            pp.pattern_scorer_weights[idx] as f64
        }

        unknown => panic!("encode_pattern_field: unknown gene name '{unknown}'"),
    }
}

fn decode_pattern_field(name: &str, v: f64, pp: &mut PatternParams) {
    match name {
        "flag_pole_mid" => pp.flag_pole_mid = v as f32,
        "flag_pole_k" => pp.flag_pole_k = v as f32,
        "flag_pole_min_bars" => pp.flag_pole_min_bars = v as usize,
        "flag_slope_center" => pp.flag_slope_center = v as f32,
        "flag_slope_sigma" => pp.flag_slope_sigma = v as f32,
        "flag_retrace_lo" => pp.flag_retrace_lo = v as f32,
        "flag_retrace_hi" => pp.flag_retrace_hi = v as f32,
        "flag_vol_mid" => pp.flag_vol_mid = v as f32,
        "flag_vol_k" => pp.flag_vol_k = v as f32,
        "flag_max_bars" => pp.flag_max_bars = v as usize,
        "flag_lookback" => pp.flag_lookback = v as usize,
        "pattern_scorer_bias" => pp.pattern_scorer_bias = v as f32,
        "pattern_alpha" => pp.pattern_alpha = v as f32,

        // flag_w0..flag_w3
        n if n.starts_with("flag_w") => {
            let idx: usize = n["flag_w".len()..].parse().unwrap_or(0);
            pp.flag_weights[idx] = v as f32;
        }

        // pat_scorer_w0..pat_scorer_w3
        n if n.starts_with("pat_scorer_w") => {
            let idx: usize = n["pat_scorer_w".len()..].parse().unwrap_or(0);
            pp.pattern_scorer_weights[idx] = v as f32;
        }

        unknown => panic!("decode_pattern_field: unknown gene name '{unknown}'"),
    }
}

// ---------------------------------------------------------------------------
// Field-level decode: f64 -> Params/SignalParams
// ---------------------------------------------------------------------------

fn decode_field(bound: &GeneBound, raw: f64, params: &mut Params) {
    // Round integers, threshold booleans before assignment.
    let v = if bound.is_integer {
        raw.round()
    } else if bound.is_boolean {
        if raw >= 0.5 { 1.0 } else { 0.0 }
    } else {
        raw
    };

    match bound.name {
        // -- Params fields --
        "rs_pct" => params.rs_pct = v as f32,
        "vol_ratio" => params.vol_ratio = v as f32,
        "max_range_pct" => params.max_range_pct = v as f32,
        "max_dist_52w" => params.max_dist_52w = v as f32,
        "min_adv" => params.min_adv = v as f32,
        "slippage_k" => params.slippage_k = v as f32,
        "min_prior_move" => params.min_prior_move = v as f32,
        "max_sma_ext" => params.max_sma_ext = v as f32,
        "risk_pct" => params.risk_pct = v as f32,
        "max_pos_pct" => params.max_pos_pct = v as f32,
        "split_frac" => params.split_frac = v as f32,
        "min_adr_pct" => params.min_adr_pct = v as f32,
        "min_consol_days" => params.min_consol_days = v as u32,
        "regime" => params.regime = v >= 0.5,

        // -- PatternParams fields --
        n if is_pattern_gene(n) => {
            let pp = params.pattern_params.get_or_insert_with(PatternParams::default);
            decode_pattern_field(bound.name, v, pp);
        }

        // -- SignalParams fields --
        _ => {
            let sp = params.signal_params.get_or_insert_with(SignalParams::default);
            decode_signal_field(bound.name, v, sp);
        }
    }
}

fn decode_signal_field(name: &str, v: f64, sp: &mut SignalParams) {
    match name {
        "window_len" => sp.window_len = v as usize,
        "candidate_threshold" => sp.candidate_threshold = v as f32,
        "l0_recompute_interval" => sp.l0_recompute_interval = v as usize,
        "pe_order" => sp.pe_order = v as usize,
        "pe_delay" => sp.pe_delay = v as usize,
        "pe_window" => sp.pe_window = v as usize,
        "bocpd_lambda" => sp.bocpd_lambda = v as f32,
        "kalman_q" => sp.kalman_q = v as f32,
        "kalman_r" => sp.kalman_r = v as f32,
        "vmd_k_modes" => sp.vmd_k_modes = v as usize,
        "vmd_alpha" => sp.vmd_alpha = v as f32,
        "vmd_sta_window" => sp.vmd_sta_window = v as usize,
        "vmd_lta_window" => sp.vmd_lta_window = v as usize,
        "knn_k" => sp.knn_k = v as usize,
        "scatter_j" => sp.scatter_j = v as usize,
        "stomp_m" => sp.stomp_m = v as usize,
        "template_len" => sp.template_len = v as usize,
        "swt_level" => sp.swt_level = v as usize,
        "hurst_window" => sp.hurst_window = v as usize,
        "hmm_n_states" => sp.hmm_n_states = v as usize,
        "rmt_window" => sp.rmt_window = v as usize,
        "conformal_coverage" => sp.conformal_coverage = v as f32,
        "conformal_window" => sp.conformal_window = v as usize,
        "vpin_n_buckets" => sp.vpin_n_buckets = v as usize,
        "scorer_bias" => sp.scorer_bias = v as f32,
        "kalman_stop_atr_mult" => sp.kalman_stop_atr_mult = v as f32,
        "bocpd_exit_threshold" => sp.bocpd_exit_threshold = v as f32,
        "vmd_entry_threshold" => sp.vmd_entry_threshold = v as f32,

        // scorer_w0..scorer_w12
        n if n.starts_with("scorer_w") => {
            let idx: usize = n["scorer_w".len()..].parse().unwrap_or(0);
            sp.scorer_weights[idx] = v as f32;
        }

        unknown => panic!("decode_signal_field: unknown gene name '{unknown}'"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_params() {
        let spec = GenomeSpec::params_spec(false);
        let params = Params::default();
        let genome = spec.encode(&params);
        assert_eq!(genome.len(), spec.dim());
        let decoded = spec.decode(&genome, &Params::default());
        assert!(
            (decoded.rs_pct as f64 - params.rs_pct as f64).abs() < 1e-4,
            "rs_pct mismatch"
        );
        assert!(
            (decoded.risk_pct as f64 - params.risk_pct as f64).abs() < 1e-4,
            "risk_pct mismatch"
        );
        assert_eq!(decoded.regime, params.regime);
        assert_eq!(decoded.min_consol_days, params.min_consol_days);
    }

    #[test]
    fn round_trip_combined() {
        let spec = GenomeSpec::combined_spec(false);
        let mut params = Params::default();
        params.signal_params = Some(SignalParams::default());
        let genome = spec.encode(&params);
        assert_eq!(genome.len(), spec.dim());
        let decoded = spec.decode(&genome, &Params::default());
        assert!(decoded.signal_params.is_some());
        let sp = decoded.signal_params.as_ref().unwrap();
        assert_eq!(sp.pe_order, SignalParams::default().pe_order);
    }

    #[test]
    fn clamp_respects_bounds() {
        let spec = GenomeSpec::params_spec(false);
        let mut genome = vec![999.0; spec.dim()];
        spec.clamp(&mut genome);
        for (i, b) in spec.bounds.iter().enumerate() {
            assert!(genome[i] <= b.max, "{} exceeded max", b.name);
            assert!(genome[i] >= b.min, "{} below min", b.name);
        }
    }

    #[test]
    fn crypto_ranges_differ() {
        let stock = GenomeSpec::params_spec(false);
        let crypto = GenomeSpec::params_spec(true);
        let rs_stock = stock.bounds.iter().find(|b| b.name == "rs_pct").unwrap();
        let rs_crypto = crypto.bounds.iter().find(|b| b.name == "rs_pct").unwrap();
        assert!(
            rs_crypto.min > rs_stock.min,
            "crypto rs_pct min should be wider"
        );
    }

    #[test]
    fn random_within_bounds() {
        let spec = GenomeSpec::combined_spec(true);
        let mut rng = rand::thread_rng();
        for _ in 0..20 {
            let genome = spec.random(&mut rng);
            assert_eq!(genome.len(), spec.dim());
            for (i, b) in spec.bounds.iter().enumerate() {
                assert!(
                    genome[i] >= b.min && genome[i] <= b.max,
                    "gene {} out of bounds: {} not in [{}, {}]",
                    b.name,
                    genome[i],
                    b.min,
                    b.max
                );
            }
        }
    }

    #[test]
    fn scorer_weights_round_trip() {
        let spec = GenomeSpec::signal_params_spec();
        let mut params = Params::default();
        let mut sp = SignalParams::default();
        sp.scorer_weights = [0.5; SCANNER_FEATURE_COUNT];
        sp.scorer_weights[0] = 2.5;
        sp.scorer_weights[12] = 0.1;
        params.signal_params = Some(sp);

        let genome = spec.encode(&params);
        let decoded = spec.decode(&genome, &Params::default());
        let dsp = decoded.signal_params.as_ref().unwrap();
        assert!((dsp.scorer_weights[0] - 2.5).abs() < 1e-4);
        assert!((dsp.scorer_weights[12] - 0.1).abs() < 1e-4);
        assert!((dsp.scorer_weights[6] - 0.5).abs() < 1e-4);
    }

    #[test]
    fn round_trip_pattern_params() {
        let spec = GenomeSpec::pattern_params_spec();
        let mut params = Params::default();
        let mut pp = PatternParams::default();
        pp.flag_pole_mid = 0.08;
        pp.flag_weights = [0.40, 0.10, 0.30, 0.20];
        pp.pattern_alpha = 0.7;
        params.pattern_params = Some(pp);

        let genome = spec.encode(&params);
        assert_eq!(genome.len(), spec.dim());
        let decoded = spec.decode(&genome, &Params::default());
        assert!(decoded.pattern_params.is_some());
        let dp = decoded.pattern_params.as_ref().unwrap();
        assert!((dp.flag_pole_mid - 0.08).abs() < 1e-4);
        assert!((dp.flag_weights[0] - 0.40).abs() < 1e-4);
        assert!((dp.pattern_alpha - 0.7).abs() < 1e-4);
    }

    #[test]
    fn round_trip_combined_with_patterns() {
        let spec = GenomeSpec::combined_with_patterns_spec(true);
        let mut params = Params::default();
        params.signal_params = Some(SignalParams::default());
        params.pattern_params = Some(PatternParams::default());
        let genome = spec.encode(&params);
        assert_eq!(genome.len(), spec.dim());
        let decoded = spec.decode(&genome, &Params::default());
        assert!(decoded.signal_params.is_some());
        assert!(decoded.pattern_params.is_some());
    }

    #[test]
    fn boolean_gene_threshold() {
        let spec = GenomeSpec::params_spec(false);
        // Regime is the last gene (index 13).
        let regime_idx = spec
            .bounds
            .iter()
            .position(|b| b.name == "regime")
            .unwrap();

        let mut genome = spec.encode(&Params::default());
        genome[regime_idx] = 0.3; // below threshold
        let decoded = spec.decode(&genome, &Params::default());
        assert!(!decoded.regime, "0.3 should decode to false");

        genome[regime_idx] = 0.7; // above threshold
        let decoded = spec.decode(&genome, &Params::default());
        assert!(decoded.regime, "0.7 should decode to true");
    }
}
