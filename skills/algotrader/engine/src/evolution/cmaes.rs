//! CMA-ES optimizer (Hansen 2016).
//!
//! Self-adaptive evolution strategy for continuous optimization in ~44 dimensions.
//! Uses faer for covariance matrix eigendecomposition.

use faer::{Mat, Side};
use rand::Rng;

use std::f64::consts::PI;

/// CMA-ES optimizer state.
pub struct CmaEs {
    dim: usize,
    // Distribution parameters
    mean: Vec<f64>,
    sigma: f64,
    // Covariance matrix and its decomposition
    c: Mat<f64>,
    b: Mat<f64>,   // eigenvectors of C
    d: Vec<f64>,   // sqrt of eigenvalues of C
    // Evolution paths
    p_sigma: Vec<f64>,
    p_c: Vec<f64>,
    // Strategy parameters (derived from dim)
    lambda: usize,
    mu: usize,
    weights: Vec<f64>,
    mu_eff: f64,
    c_sigma: f64,
    d_sigma: f64,
    c_c: f64,
    c_1: f64,
    c_mu: f64,
    // Bookkeeping
    generation: usize,
    eigendecomp_gen: usize,
}

#[allow(clippy::needless_range_loop)]
impl CmaEs {
    /// Create a new CMA-ES optimizer.
    ///
    /// All strategy parameters are computed from `dim` following the Hansen 2016
    /// defaults. The covariance matrix starts as identity.
    pub fn new(dim: usize, initial_mean: Vec<f64>, initial_sigma: f64) -> Self {
        assert!(dim >= 1, "dimension must be at least 1");
        assert_eq!(initial_mean.len(), dim);

        // Population and parent sizes.
        let lambda = (4 + (3.0 * (dim as f64).ln()).floor() as usize).max(6);
        let mu = lambda / 2;

        // Recombination weights: w_i = ln(mu + 0.5) - ln(i + 1), then normalize.
        let raw_weights: Vec<f64> = (0..mu)
            .map(|i| (mu as f64 + 0.5).ln() - ((i + 1) as f64).ln())
            .collect();
        let w_sum: f64 = raw_weights.iter().sum();
        let weights: Vec<f64> = raw_weights.iter().map(|w| w / w_sum).collect();

        let w_sq_sum: f64 = weights.iter().map(|w| w * w).sum();
        let mu_eff = 1.0 / w_sq_sum;

        // Step-size control.
        let c_sigma = (mu_eff + 2.0) / (dim as f64 + mu_eff + 5.0);
        let d_sigma = 1.0
            + 2.0 * ((mu_eff - 1.0) / (dim as f64 + 1.0)).sqrt().max(0.0).max(0.0 - 1.0)
            + c_sigma;

        // Covariance adaptation.
        let c_c = (4.0 + mu_eff / dim as f64) / (dim as f64 + 4.0 + 2.0 * mu_eff / dim as f64);
        let c_1 = 2.0 / ((dim as f64 + 1.3).powi(2) + mu_eff);
        let c_mu_raw =
            2.0 * (mu_eff - 2.0 + 1.0 / mu_eff) / ((dim as f64 + 2.0).powi(2) + mu_eff);
        let c_mu = c_mu_raw.min(1.0 - c_1);

        // Initialize C = I, B = I, D = ones.
        let c = Mat::<f64>::from_fn(dim, dim, |i, j| if i == j { 1.0 } else { 0.0 });
        let b = Mat::<f64>::from_fn(dim, dim, |i, j| if i == j { 1.0 } else { 0.0 });
        let d = vec![1.0; dim];

        Self {
            dim,
            mean: initial_mean,
            sigma: initial_sigma,
            c,
            b,
            d,
            p_sigma: vec![0.0; dim],
            p_c: vec![0.0; dim],
            lambda,
            mu,
            weights,
            mu_eff,
            c_sigma,
            d_sigma,
            c_c,
            c_1,
            c_mu,
            generation: 0,
            eigendecomp_gen: 0,
        }
    }

    /// Sample `lambda` offspring from the current distribution.
    ///
    /// Each offspring: x_i = mean + sigma * B * diag(D) * z_i, where z_i ~ N(0, I).
    pub fn ask(&mut self, rng: &mut impl Rng) -> Vec<Vec<f64>> {
        let mut population = Vec::with_capacity(self.lambda);

        for _ in 0..self.lambda {
            // Sample z ~ N(0, I) using Box-Muller.
            let z = sample_normal(self.dim, rng);

            // y = B * diag(D) * z
            let mut y = vec![0.0; self.dim];
            for i in 0..self.dim {
                let mut sum = 0.0;
                for j in 0..self.dim {
                    sum += self.b.read(i, j) * self.d[j] * z[j];
                }
                y[i] = sum;
            }

            // x = mean + sigma * y
            let x: Vec<f64> = (0..self.dim)
                .map(|i| self.mean[i] + self.sigma * y[i])
                .collect();
            population.push(x);
        }

        population
    }

    /// Update the distribution from evaluated offspring.
    ///
    /// `ranked_population` must be sorted by fitness, best first. Only the top
    /// `mu` individuals are used for the update.
    pub fn tell(&mut self, ranked_population: &[(Vec<f64>, f64)]) {
        self.generation += 1;
        let n = self.dim;

        let old_mean = self.mean.clone();

        // --- 1. Update mean (weighted recombination of top mu) ---
        let mut new_mean = vec![0.0; n];
        for (i, w) in self.weights.iter().enumerate() {
            let x = &ranked_population[i].0;
            for j in 0..n {
                new_mean[j] += w * x[j];
            }
        }
        self.mean = new_mean;

        // Displacement scaled by sigma.
        let mean_diff: Vec<f64> = (0..n)
            .map(|i| (self.mean[i] - old_mean[i]) / self.sigma)
            .collect();

        // --- 2. Update sigma (CSA) ---
        // invsqrt_c = B * diag(1/D) * B^T
        let invsqrt_c_times_diff = self.apply_invsqrt_c(&mean_diff);

        let cs = self.c_sigma;
        let sqrt_cs_factor = (cs * (2.0 - cs) * self.mu_eff).sqrt();
        for i in 0..n {
            self.p_sigma[i] = (1.0 - cs) * self.p_sigma[i]
                + sqrt_cs_factor * invsqrt_c_times_diff[i];
        }

        let ps_norm = vec_norm(&self.p_sigma);
        let expected_norm = expected_chi(n);
        self.sigma *= ((cs / self.d_sigma) * (ps_norm / expected_norm - 1.0)).exp();
        // Clamp sigma to prevent explosion on flat landscapes.
        self.sigma = self.sigma.clamp(1e-20, 1e2);

        // --- 3. Update covariance ---
        // h_sigma: indicator for stalling detection.
        let gen = self.generation as f64;
        let threshold = (1.4 + 2.0 / (n as f64 + 1.0)) * expected_norm;
        let discount = (1.0 - (1.0 - cs).powf(2.0 * gen)).sqrt();
        let h_sigma = if discount > 0.0 && ps_norm / discount < threshold {
            1.0
        } else {
            0.0
        };

        // Update p_c.
        let cc = self.c_c;
        let sqrt_cc_factor = (cc * (2.0 - cc) * self.mu_eff).sqrt();
        for i in 0..n {
            self.p_c[i] =
                (1.0 - cc) * self.p_c[i] + h_sigma * sqrt_cc_factor * mean_diff[i];
        }

        // Rank-one update: p_c * p_c^T.
        // Rank-mu update: weighted sum of y_i * y_i^T.
        let c1 = self.c_1;
        let cmu = self.c_mu;

        // Pre-compute y_i = (x_i - old_mean) / sigma for top mu individuals.
        let ys: Vec<Vec<f64>> = (0..self.mu)
            .map(|i| {
                let x = &ranked_population[i].0;
                (0..n)
                    .map(|j| (x[j] - old_mean[j]) / self.sigma)
                    .collect()
            })
            .collect();

        // C = (1 - c_1 - c_mu) * C + c_1 * (p_c * p_c^T) + c_mu * sum(w_i * y_i * y_i^T)
        let scale = 1.0 - c1 - cmu;
        for i in 0..n {
            for j in 0..=i {
                let mut val = scale * self.c.read(i, j);

                // Rank-one.
                val += c1 * self.p_c[i] * self.p_c[j];

                // Rank-mu.
                let mut rank_mu_val = 0.0;
                for (k, w) in self.weights.iter().enumerate() {
                    rank_mu_val += w * ys[k][i] * ys[k][j];
                }
                val += cmu * rank_mu_val;

                self.c.write(i, j, val);
                if i != j {
                    self.c.write(j, i, val); // symmetric
                }
            }
        }

        // --- 4. Eigendecompose C periodically ---
        let decomp_interval = (n / 10).max(1);
        if self.generation - self.eigendecomp_gen >= decomp_interval {
            self.update_eigenbasis();
            self.eigendecomp_gen = self.generation;
        }
    }

    /// Current step size.
    pub fn sigma(&self) -> f64 {
        self.sigma
    }

    /// Current generation count.
    pub fn generation(&self) -> usize {
        self.generation
    }

    /// Current distribution mean.
    pub fn mean(&self) -> &[f64] {
        &self.mean
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Compute B * diag(1/D) * B^T * v  (apply the inverse square root of C).
    fn apply_invsqrt_c(&self, v: &[f64]) -> Vec<f64> {
        let n = self.dim;

        // Step 1: tmp = B^T * v
        let mut tmp = vec![0.0; n];
        for i in 0..n {
            let mut sum = 0.0;
            for j in 0..n {
                sum += self.b.read(j, i) * v[j];
            }
            tmp[i] = sum;
        }

        // Step 2: tmp[i] /= d[i]
        for i in 0..n {
            if self.d[i] > 1e-20 {
                tmp[i] /= self.d[i];
            }
        }

        // Step 3: result = B * tmp
        let mut result = vec![0.0; n];
        for i in 0..n {
            let mut sum = 0.0;
            for j in 0..n {
                sum += self.b.read(i, j) * tmp[j];
            }
            result[i] = sum;
        }

        result
    }

    /// Eigendecompose the covariance matrix and update B, D.
    fn update_eigenbasis(&mut self) {
        let evd = self.c.selfadjoint_eigendecomposition(Side::Lower);
        let eigvals = evd.s().column_vector();
        let eigvecs = evd.u();

        let n = self.dim;
        for i in 0..n {
            // Clamp eigenvalues to avoid negative sqrt from numerical noise.
            let lam = eigvals.read(i).max(1e-20);
            self.d[i] = lam.sqrt();
        }

        self.b = Mat::<f64>::from_fn(n, n, |i, j| eigvecs.read(i, j));
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

/// Sample `dim` standard normal values using Box-Muller transform.
fn sample_normal(dim: usize, rng: &mut impl Rng) -> Vec<f64> {
    let mut z = Vec::with_capacity(dim);
    let mut i = 0;
    while i < dim {
        let u1: f64 = rng.gen();
        let u2: f64 = rng.gen();
        // Avoid log(0).
        let u1 = if u1 < 1e-300 { 1e-300 } else { u1 };
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * PI * u2;
        z.push(r * theta.cos());
        if i + 1 < dim {
            z.push(r * theta.sin());
        }
        i += 2;
    }
    z.truncate(dim);
    z
}

/// Euclidean norm of a vector.
fn vec_norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// Expected value of ||N(0, I)|| for dimension `n` (chi distribution approximation).
fn expected_chi(n: usize) -> f64 {
    let n = n as f64;
    n.sqrt() * (1.0 - 1.0 / (4.0 * n) + 1.0 / (21.0 * n * n))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strategy_params_from_dim() {
        let cma = CmaEs::new(10, vec![0.0; 10], 1.0);
        assert!(cma.lambda >= 6);
        assert!(cma.mu > 0 && cma.mu <= cma.lambda);
        assert!(cma.c_sigma > 0.0 && cma.c_sigma < 1.0);
    }

    #[test]
    fn ask_produces_correct_count() {
        let mut cma = CmaEs::new(5, vec![0.0; 5], 1.0);
        let mut rng = rand::thread_rng();
        let pop = cma.ask(&mut rng);
        assert_eq!(pop.len(), cma.lambda);
        for ind in &pop {
            assert_eq!(ind.len(), 5);
        }
    }

    #[test]
    fn rosenbrock_2d_converges() {
        // CMA-ES should find near (1,1) on the 2D Rosenbrock function.
        // f(x,y) = (1-x)^2 + 100*(y-x^2)^2, minimum at (1,1).
        let mut cma = CmaEs::new(2, vec![0.0, 0.0], 0.5);
        let mut rng = rand::thread_rng();

        for _ in 0..200 {
            let pop = cma.ask(&mut rng);
            let mut scored: Vec<(Vec<f64>, f64)> = pop
                .into_iter()
                .map(|x| {
                    let f = (1.0 - x[0]).powi(2) + 100.0 * (x[1] - x[0] * x[0]).powi(2);
                    (x, -f) // negate: tell() expects higher = better
                })
                .collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            cma.tell(&scored);
        }

        let m = &cma.mean;
        assert!(
            (m[0] - 1.0).abs() < 0.3,
            "x should be near 1, got {}",
            m[0]
        );
        assert!(
            (m[1] - 1.0).abs() < 0.3,
            "y should be near 1, got {}",
            m[1]
        );
    }

    #[test]
    fn sigma_adapts() {
        // On a sphere function, sigma should decrease as we approach the optimum.
        let mut cma = CmaEs::new(3, vec![5.0; 3], 2.0);
        let mut rng = rand::thread_rng();
        let initial_sigma = cma.sigma();

        for _ in 0..50 {
            let pop = cma.ask(&mut rng);
            let mut scored: Vec<(Vec<f64>, f64)> = pop
                .into_iter()
                .map(|x| {
                    let f: f64 = x.iter().map(|xi| xi * xi).sum();
                    (x, -f)
                })
                .collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            cma.tell(&scored);
        }

        assert!(
            cma.sigma() < initial_sigma,
            "sigma should decrease: initial={}, current={}",
            initial_sigma,
            cma.sigma()
        );
    }
}
