//! Flat collection of all signal-processing algorithms.
//!
//! Previously split across layer0/layer1/layer2 directories with fixed roles.
//! Now composable: any algorithm can be wired into any pipeline stage.

pub mod bocpd;
pub mod conformal;
pub mod hmm;
pub mod hurst;
pub mod kalman;
pub mod knn_anomaly;
pub mod kronos;
pub mod perm_entropy;
pub mod renyi;
pub mod rmt;
pub mod scattering;
pub mod stomp;
pub mod swt;
pub mod template;
pub mod transfer;
pub mod vmd;
pub mod vpin;
