//! Simple GA optimizer (port of scripts/evolve.py).
//!
//! Tournament selection, uniform crossover, Gaussian mutation with adaptive
//! sigma, elitism, stagnation injection, and cataclysm reset.

use rand::Rng;

/// GA optimizer state.
pub struct Ga {
    _dim: usize,
    pop_size: usize,
    elite_frac: f32,
    // Adaptive mutation state
    base_sigma: f64,
    sigma: f64,
    stale_gens: usize,
    stale_limit: usize,
    cataclysm_limit: usize,
    // Tracking
    best_fitness: f64,
    generation: usize,
}

impl Ga {
    /// Create a new GA optimizer.
    ///
    /// `elite_frac` controls what fraction of the population survives
    /// unchanged into the next generation (typically 0.2-0.3).
    pub fn new(dim: usize, pop_size: usize, elite_frac: f32) -> Self {
        Self {
            _dim: dim,
            pop_size,
            elite_frac,
            base_sigma: 0.15,
            sigma: 0.15,
            stale_gens: 0,
            stale_limit: 5,
            cataclysm_limit: 12,
            best_fitness: -999.0,
            generation: 0,
        }
    }

    /// Produce the next generation from a scored, sorted population.
    ///
    /// `population` must be sorted by fitness (best first). Each entry is
    /// `(genome, fitness)`. `bounds` provides `(min, max, is_integer, is_boolean)`
    /// per gene.
    ///
    /// Returns `pop_size` new genomes for the next generation.
    pub fn evolve_step(
        &mut self,
        population: &[(Vec<f64>, f64)],
        bounds: &[(f64, f64, bool, bool)],
        rng: &mut impl Rng,
    ) -> Vec<Vec<f64>> {
        self.generation += 1;
        let top_fitness = population[0].1;

        // Track stagnation and adapt mutation strength.
        if top_fitness > self.best_fitness {
            self.best_fitness = top_fitness;
            self.stale_gens = 0;
            self.sigma = self.base_sigma;
        } else {
            self.stale_gens += 1;
            self.sigma = self.base_sigma * (1.0 + self.stale_gens as f64 * 0.3);
        }

        // Start next generation with the elite.
        let elite_count = (self.pop_size as f32 * self.elite_frac).round() as usize;
        let elite_count = elite_count.max(1).min(population.len());
        let mut next_pop: Vec<Vec<f64>> = population[..elite_count]
            .iter()
            .map(|(g, _)| g.clone())
            .collect();

        // Injection on stagnation.
        if self.stale_gens >= self.cataclysm_limit {
            // Cataclysm: nuke half the slots with random individuals.
            let n_random = self.pop_size / 2;
            for _ in 0..n_random {
                next_pop.push(random_individual(bounds, rng));
            }
            self.stale_gens = 0;
            self.sigma = self.base_sigma * 2.0;
        } else if self.stale_gens >= self.stale_limit {
            // Mild injection: 30% random individuals.
            let n_random = (self.pop_size as f64 * 0.3) as usize;
            for _ in 0..n_random {
                next_pop.push(random_individual(bounds, rng));
            }
        }

        // Fill remaining slots via tournament + crossover + mutation.
        while next_pop.len() < self.pop_size {
            let a = tournament_select(population, 3, rng);
            let b = tournament_select(population, 3, rng);
            let mut child = crossover(&a, &b, rng);
            mutate(&mut child, bounds, 0.3, self.sigma, rng);
            next_pop.push(child);
        }

        next_pop.truncate(self.pop_size);
        next_pop
    }

    /// Current adaptive mutation sigma.
    pub fn sigma(&self) -> f64 {
        self.sigma
    }

    /// Current generation count.
    pub fn generation(&self) -> usize {
        self.generation
    }

    /// How many generations without fitness improvement.
    pub fn stale_gens(&self) -> usize {
        self.stale_gens
    }
}

// ---------------------------------------------------------------------------
// GA operators
// ---------------------------------------------------------------------------

/// Pick `k` random individuals from `scored` (sorted best-first), return the
/// best one's genome. Because the slice is sorted, the individual with the
/// smallest index wins.
fn tournament_select<R: Rng>(scored: &[(Vec<f64>, f64)], k: usize, rng: &mut R) -> Vec<f64> {
    let n = scored.len();
    let k = k.min(n);
    let mut best_idx = rng.gen_range(0..n);
    for _ in 1..k {
        let idx = rng.gen_range(0..n);
        if scored[idx].1 > scored[best_idx].1 {
            best_idx = idx;
        }
    }
    scored[best_idx].0.clone()
}

/// Uniform crossover: for each gene, pick from `a` or `b` with 50% probability.
fn crossover<R: Rng>(a: &[f64], b: &[f64], rng: &mut R) -> Vec<f64> {
    a.iter()
        .zip(b.iter())
        .map(|(&va, &vb)| if rng.gen_bool(0.5) { va } else { vb })
        .collect()
}

/// Gaussian mutation with per-gene probability `rate`.
///
/// For each gene with probability `rate`: add N(0, sigma * (max - min)).
/// Then clamp to bounds, round integers, threshold booleans at 0.5.
fn mutate<R: Rng>(
    child: &mut [f64],
    bounds: &[(f64, f64, bool, bool)],
    rate: f64,
    sigma: f64,
    rng: &mut R,
) {
    for (i, &(lo, hi, is_int, is_bool)) in bounds.iter().enumerate() {
        if rng.gen::<f64>() < rate {
            let span = hi - lo;
            // Box-Muller for a single normal sample.
            let u1: f64 = rng.gen::<f64>().max(1e-300);
            let u2: f64 = rng.gen();
            let noise = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();
            child[i] += sigma * span * noise;
        }
        child[i] = child[i].clamp(lo, hi);
        if is_int {
            child[i] = child[i].round();
        }
        if is_bool {
            child[i] = if child[i] >= 0.5 { 1.0 } else { 0.0 };
        }
    }
}

/// Generate a random individual uniformly sampled within bounds.
///
/// `pub(super)` so the parent module can use it for seeding initial populations.
pub(super) fn random_individual<R: Rng>(bounds: &[(f64, f64, bool, bool)], rng: &mut R) -> Vec<f64> {
    bounds
        .iter()
        .map(|&(lo, hi, is_int, is_bool)| {
            if is_bool {
                if rng.gen_bool(0.5) { 1.0 } else { 0.0 }
            } else {
                let v = rng.gen_range(lo..=hi);
                if is_int { v.round() } else { v }
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn population_size_preserved() {
        let mut ga = Ga::new(3, 20, 0.25);
        let mut rng = rand::thread_rng();
        let bounds = vec![(0.0, 1.0, false, false); 3];

        // Create initial scored population (sorted: best first).
        let scored: Vec<(Vec<f64>, f64)> = (0..20).map(|i| (vec![0.5; 3], -(i as f64))).collect();

        let next = ga.evolve_step(&scored, &bounds, &mut rng);
        assert_eq!(next.len(), 20);
    }

    #[test]
    fn stagnation_increases_sigma() {
        let mut ga = Ga::new(3, 10, 0.25);
        let mut rng = rand::thread_rng();
        let bounds = vec![(0.0, 1.0, false, false); 3];
        let initial_sigma = ga.sigma();

        // Feed same best fitness repeatedly.
        for _ in 0..6 {
            let scored: Vec<(Vec<f64>, f64)> =
                (0..10).map(|_| (vec![0.5; 3], 1.0)).collect();
            let _ = ga.evolve_step(&scored, &bounds, &mut rng);
        }

        assert!(
            ga.sigma() > initial_sigma,
            "sigma should increase with stagnation"
        );
        assert!(ga.stale_gens() >= 5);
    }

    #[test]
    fn improvement_resets_stale() {
        let mut ga = Ga::new(3, 10, 0.25);
        let mut rng = rand::thread_rng();
        let bounds = vec![(0.0, 1.0, false, false); 3];

        // Stagnate.
        for _ in 0..3 {
            let scored: Vec<(Vec<f64>, f64)> =
                (0..10).map(|_| (vec![0.5; 3], 1.0)).collect();
            let _ = ga.evolve_step(&scored, &bounds, &mut rng);
        }
        assert!(ga.stale_gens() > 0);

        // Improve.
        let scored: Vec<(Vec<f64>, f64)> = (0..10).map(|_| (vec![0.5; 3], 5.0)).collect();
        let _ = ga.evolve_step(&scored, &bounds, &mut rng);
        assert_eq!(ga.stale_gens(), 0);
    }

    #[test]
    fn sphere_converges() {
        // GA should move toward origin on sphere function.
        let dim = 5;
        let pop_size = 50;
        let mut ga = Ga::new(dim, pop_size, 0.25);
        let mut rng = rand::thread_rng();
        let bounds: Vec<(f64, f64, bool, bool)> = vec![(-5.0, 5.0, false, false); dim];

        // Random initial population.
        let mut pop: Vec<Vec<f64>> = (0..pop_size)
            .map(|_| random_individual(&bounds, &mut rng))
            .collect();

        for _ in 0..100 {
            let mut scored: Vec<(Vec<f64>, f64)> = pop
                .into_iter()
                .map(|x| {
                    let f: f64 = x.iter().map(|xi| xi * xi).sum();
                    (x, -f)
                })
                .collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            pop = ga.evolve_step(&scored, &bounds, &mut rng);
        }

        // Score the final population to find the best.
        let mut scored: Vec<(Vec<f64>, f64)> = pop
            .into_iter()
            .map(|x| {
                let f: f64 = x.iter().map(|xi| xi * xi).sum();
                (x, -f)
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let best = &scored[0].0;
        let dist: f64 = best.iter().map(|x| x * x).sum::<f64>().sqrt();
        assert!(dist < 2.0, "best should be near origin, dist={dist}");
    }

    #[test]
    fn integer_and_boolean_genes() {
        let mut rng = rand::thread_rng();
        let bounds = vec![
            (1.0, 10.0, true, false),  // integer
            (0.0, 1.0, false, true),   // boolean
            (0.0, 1.0, false, false),  // continuous
        ];

        for _ in 0..20 {
            let ind = random_individual(&bounds, &mut rng);
            assert_eq!(ind[0], ind[0].round(), "integer gene should be rounded");
            assert!(
                ind[1] == 0.0 || ind[1] == 1.0,
                "boolean gene should be 0 or 1"
            );
        }
    }
}
