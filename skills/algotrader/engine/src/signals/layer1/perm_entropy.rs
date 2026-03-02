//! 1.1 Permutation Entropy — complexity filter for consolidation detection.
//!
//! PE measures the ordinal complexity of a time series.  A declining PE
//! indicates the series is becoming more structured (ordered), which is
//! exactly what happens during a consolidation / coiling phase.
//!
//! Returns PE in [0, 1]: 0 = perfectly ordered, 1 = random.

/// Compute permutation entropy of embedding dimension `order` with time delay
/// `delay` over the given `window`.
///
/// Uses a flat count array sized to `order!` (max supported order = 5, giving
/// 120 slots).  Ordinal patterns are encoded as a Lehmer code index so each
/// pattern maps to a unique slot without hashing.
///
/// If the window is too short for even one embedding vector the function
/// returns 1.0 (maximum entropy -- no information).
pub fn compute_perm_entropy(window: &[f32], order: usize, delay: usize) -> f32 {
    // Minimum length required: (order - 1) * delay + 1.
    let embed_len = (order - 1) * delay + 1;
    if window.len() < embed_len || order < 2 {
        return 1.0;
    }

    let n_perms = factorial(order);
    // Stack array for order <= 5 (120 slots); heap-allocate beyond that.
    let mut counts = vec![0u32; n_perms];

    let n_patterns = window.len() - embed_len + 1;

    for i in 0..n_patterns {
        let idx = lehmer_index(window, i, order, delay);
        if idx < n_perms {
            counts[idx] += 1;
        }
    }

    // Shannon entropy of the distribution, normalised by ln(order!).
    let total = n_patterns as f32;
    let ln_norm = (n_perms as f32).ln();
    if ln_norm == 0.0 {
        return 1.0;
    }

    let mut entropy = 0.0_f32;
    for &c in &counts {
        if c > 0 {
            let p = c as f32 / total;
            entropy -= p * p.ln();
        }
    }

    (entropy / ln_norm).clamp(0.0, 1.0)
}

/// Encode the ordinal pattern starting at `start` into a Lehmer code index.
///
/// For each position in the embedding vector, count how many subsequent
/// elements are smaller -- this gives the factoradic digit.  Convert the
/// factoradic representation to a single integer.
fn lehmer_index(window: &[f32], start: usize, order: usize, delay: usize) -> usize {
    let mut index = 0usize;
    for i in 0..order {
        let mut rank = 0usize;
        let vi = window[start + i * delay];
        for j in (i + 1)..order {
            if window[start + j * delay] < vi {
                rank += 1;
            }
        }
        index = index * (order - i) + rank;
    }
    index
}

fn factorial(n: usize) -> usize {
    (1..=n).product()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfectly_ordered_ascending() {
        // Strictly ascending series -> one dominant pattern -> low entropy.
        let window: Vec<f32> = (0..30).map(|i| i as f32).collect();
        let pe = compute_perm_entropy(&window, 5, 1);
        assert!(pe < 0.05, "ascending series should have near-zero PE, got {pe}");
    }

    #[test]
    fn short_window_returns_max() {
        let window = [1.0, 2.0];
        assert_eq!(compute_perm_entropy(&window, 5, 1), 1.0);
    }
}
