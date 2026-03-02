//! Feature vector type shared across scanner and scorer layers.

/// Number of independent features produced by the Layer 1 scanner.
/// Matches the field count of `ScannerOutput` (see `layer1/mod.rs`).
pub const SCANNER_FEATURE_COUNT: usize = 13;

/// Fixed-size feature array used by the scorer and arena scratch buffers.
pub type FeatureVec = [f32; SCANNER_FEATURE_COUNT];
