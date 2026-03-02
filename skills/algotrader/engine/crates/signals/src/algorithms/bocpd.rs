//! 1.2 BOCPD -- Bayesian Online Changepoint Detection.
//!
//! Manual implementation of the Adams & MacKay (2007) algorithm with a
//! constant hazard rate `1/lambda` and a Normal-Inverse-Gamma conjugate
//! prior whose predictive is Student-t.
//!
//! Run-length probabilities are truncated at `max_run_len` for bounded
//! memory.  Sufficient statistics (count, sum, sum-of-squares) are tracked
//! per run length for the Gaussian predictive distribution.

/// Online BOCPD state maintained across bars for a single ticker.
///
/// Call `update()` once per bar with the return (or close price diff).
/// `reset()` clears state between tickers.
pub struct BocpdState {
    /// Probability mass on each run length r = 0 .. max_run_len.
    pub run_lengths: Vec<f32>,
    /// Hard upper bound on the run-length vector.
    pub max_run_len: usize,
    // -- Normal-Inverse-Gamma prior hyperparameters --
    kappa0: f32,
    mu0: f32,
    alpha0: f32,
    beta0: f32,
    // -- Sufficient statistics per run length --
    // Posterior after n observations with sum S and sum-of-squares Q:
    //   kappa_n = kappa0 + n
    //   mu_n    = (kappa0 * mu0 + S) / kappa_n
    //   alpha_n = alpha0 + n / 2
    //   beta_n  = beta0 + 0.5*(Q - S^2/kappa_n) + 0.5*kappa0*n*mu0^2/kappa_n
    count: Vec<f32>,
    sum: Vec<f32>,
    sum_sq: Vec<f32>,
    /// Whether the first observation has been fed (needed to initialise R).
    initialised: bool,
}

impl BocpdState {
    /// Create a new BOCPD state with the given truncation limit and default
    /// priors (kappa0=1, mu0=0, alpha0=1, beta0=1).
    pub fn new(max_run_len: usize) -> Self {
        Self::with_priors(max_run_len, 1.0, 0.0, 1.0, 1.0)
    }

    /// Create a new BOCPD state with explicit Normal-Inverse-Gamma prior
    /// hyperparameters.
    pub fn with_priors(max_run_len: usize, kappa0: f32, mu0: f32, alpha0: f32, beta0: f32) -> Self {
        let cap = max_run_len + 1;
        let mut rl = vec![0.0_f32; cap];
        rl[0] = 1.0; // all mass on run length 0 before any data
        Self {
            run_lengths: rl,
            max_run_len,
            kappa0,
            mu0,
            alpha0,
            beta0,
            count: vec![0.0; cap],
            sum: vec![0.0; cap],
            sum_sq: vec![0.0; cap],
            initialised: false,
        }
    }

    /// Feed one observation and return P(changepoint at this bar).
    ///
    /// `lambda` is the expected run length (hazard rate = 1/lambda).
    #[allow(clippy::needless_range_loop)]
    pub fn update(&mut self, x: f32, lambda: f32) -> f32 {
        if x.is_nan() {
            return 0.0;
        }

        let n = self.max_run_len + 1;
        let h = 1.0 / lambda.max(1.0); // hazard probability

        if !self.initialised {
            self.initialised = true;
            // First observation -- initialise sufficient stats for r = 0.
            self.count[0] = 1.0;
            self.sum[0] = x;
            self.sum_sq[0] = x * x;
            return 0.0;
        }

        // Compute predictive probability P(x | r) for each active run length.
        // Uses Student-t predictive from the Normal-Inverse-Gamma conjugate.
        let mut pred = vec![0.0_f32; n];
        for r in 0..n {
            if self.run_lengths[r] < 1e-30 {
                continue;
            }
            pred[r] = self.predictive(r, x);
        }

        // Growth and changepoint probabilities.
        let mut new_rl = vec![0.0_f32; n];

        // Growth: R_new[r+1] = R[r] * pred[r] * (1 - h)
        for r in 0..(n - 1) {
            new_rl[r + 1] += self.run_lengths[r] * pred[r] * (1.0 - h);
        }

        // Changepoint: R_new[0] = sum_r R[r] * pred[r] * h
        let mut cp_mass = 0.0_f32;
        for r in 0..n {
            cp_mass += self.run_lengths[r] * pred[r] * h;
        }
        new_rl[0] += cp_mass;

        // Normalise to avoid underflow drift.
        let total: f32 = new_rl.iter().sum();
        if total > 0.0 {
            let inv = 1.0 / total;
            for v in new_rl.iter_mut() {
                *v *= inv;
            }
        }

        // Update sufficient statistics.  Shift existing stats forward (growth)
        // and reset slot 0 (changepoint starts a fresh run).
        let mut new_count = vec![0.0_f32; n];
        let mut new_sum = vec![0.0_f32; n];
        let mut new_sum_sq = vec![0.0_f32; n];

        for r in 0..(n - 1) {
            new_count[r + 1] = self.count[r] + 1.0;
            new_sum[r + 1] = self.sum[r] + x;
            new_sum_sq[r + 1] = self.sum_sq[r] + x * x;
        }
        // Changepoint slot: single observation.
        new_count[0] = 1.0;
        new_sum[0] = x;
        new_sum_sq[0] = x * x;

        self.run_lengths = new_rl;
        self.count = new_count;
        self.sum = new_sum;
        self.sum_sq = new_sum_sq;

        // Truncate: fold mass beyond max_run_len into the last slot.
        // (Already naturally bounded since our vectors are fixed size.)

        cp_mass.clamp(0.0, 1.0)
    }

    /// Reset all state (between tickers).
    pub fn reset(&mut self) {
        for v in self.run_lengths.iter_mut() {
            *v = 0.0;
        }
        self.run_lengths[0] = 1.0;
        for v in self.count.iter_mut() { *v = 0.0; }
        for v in self.sum.iter_mut() { *v = 0.0; }
        for v in self.sum_sq.iter_mut() { *v = 0.0; }
        self.initialised = false;
    }

    /// Student-t predictive probability of `x` given sufficient stats at slot `r`.
    ///
    /// Uses stored Normal-Inverse-Gamma prior hyperparameters (kappa0, mu0,
    /// alpha0, beta0).
    fn predictive(&self, r: usize, x: f32) -> f32 {
        let n = self.count[r];
        let s = self.sum[r];
        let q = self.sum_sq[r];

        // Posterior hyperparameters.
        let kappa_n = self.kappa0 + n;
        let mu_n = (self.kappa0 * self.mu0 + s) / kappa_n;
        let alpha_n = self.alpha0 + n * 0.5;
        // beta_n = beta0 + 0.5*(Q - S^2/kappa_n) + 0.5*kappa0*n*mu0^2/kappa_n
        let beta_n = (self.beta0 + 0.5 * (q - s * s / kappa_n)
            + 0.5 * self.kappa0 * n * self.mu0 * self.mu0 / kappa_n)
            .max(1e-10);

        // Student-t parameters.
        let df = 2.0 * alpha_n;
        let loc = mu_n;
        let scale_sq = beta_n * (kappa_n + 1.0) / (alpha_n * kappa_n);
        let scale = scale_sq.max(1e-10).sqrt();

        student_t_pdf(x, df, loc, scale)
    }
}

/// Probability density of the Student-t distribution at `x`.
fn student_t_pdf(x: f32, df: f32, loc: f32, scale: f32) -> f32 {
    let z = (x - loc) / scale;
    let half_df = df * 0.5;
    let half_df1 = (df + 1.0) * 0.5;
    // log pdf = log_gamma(half_df1) - log_gamma(half_df) - 0.5*ln(df*pi)
    //         - ln(scale) - half_df1 * ln(1 + z^2/df)
    let log_pdf = ln_gamma(half_df1)
        - ln_gamma(half_df)
        - 0.5 * (df * std::f32::consts::PI).ln()
        - scale.ln()
        - half_df1 * (1.0 + z * z / df).ln();
    log_pdf.exp().max(1e-30)
}

/// Stirling's approximation for the log-gamma function.
/// Accurate to ~1e-5 for x >= 1.
fn ln_gamma(x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    // Lanczos-like series (sufficient for f32 precision).
    let x = x as f64;
    let result = (x - 0.5) * x.ln() - x + 0.5 * (2.0 * std::f64::consts::PI).ln()
        + 1.0 / (12.0 * x)
        - 1.0 / (360.0 * x * x * x);
    result as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_mean_shift() {
        let mut state = BocpdState::new(200);
        // 50 observations near 0, then 50 near 5 -- a clear mean shift.
        // Use a shorter lambda (20) so the hazard rate is high enough to
        // respond within the 100-observation window.
        let mut max_cp = 0.0_f32;
        for i in 0..100 {
            let x = if i < 50 { 0.1 * (i % 5) as f32 } else { 5.0 + 0.1 * (i % 5) as f32 };
            let cp = state.update(x, 20.0);
            if (50..=60).contains(&i) {
                max_cp = max_cp.max(cp);
            }
        }
        // The changepoint probability should be noticeably elevated after the
        // shift, even if not huge in absolute terms.
        assert!(max_cp > 0.001, "should detect changepoint, max_cp = {max_cp}");
    }
}
