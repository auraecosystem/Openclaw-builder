//! 1.7 VPIN -- Volume-synchronised Probability of Informed Trading.
//!
//! Simplified implementation for bar data: uses the tick rule (close > prev
//! close = buy) as a proxy when tick-level data is unavailable.  Volume is
//! bucketed into `n_buckets` equal-volume bins, and the mean absolute
//! imbalance across buckets gives the VPIN estimate in [0, 1].

/// Compute VPIN from close prices and bar volumes.
///
/// # Arguments
/// * `close` -- close prices, length N (N >= 2).
/// * `volume` -- bar volumes, same length as `close`.
/// * `n_buckets` -- number of equal-volume buckets for the rolling estimate.
///
/// # Returns
/// VPIN in [0, 1].  Returns 0.0 if inputs are too short or volume is zero.
pub fn compute_vpin(close: &[f32], volume: &[f32], n_buckets: usize) -> f32 {
    if close.len() < 2 || close.len() != volume.len() || n_buckets == 0 {
        return 0.0;
    }

    // Total volume to determine bucket size.
    let total_vol: f32 = volume.iter().sum();
    if total_vol <= 0.0 {
        return 0.0;
    }
    let bucket_vol = total_vol / n_buckets as f32;
    if bucket_vol <= 0.0 {
        return 0.0;
    }

    // Walk bars, classify as buy/sell, accumulate into buckets.
    let mut bucket_buy = 0.0_f32;
    let mut bucket_sell = 0.0_f32;
    let mut bucket_remaining = bucket_vol;
    let mut imbalances = Vec::with_capacity(n_buckets);

    for i in 1..close.len() {
        let bar_vol = volume[i];
        if bar_vol <= 0.0 {
            continue;
        }

        // Tick rule: classify entire bar volume as buy or sell.
        let is_buy = close[i] > close[i - 1];
        let mut remaining = bar_vol;

        while remaining > 0.0 {
            let fill = remaining.min(bucket_remaining);
            if is_buy {
                bucket_buy += fill;
            } else {
                bucket_sell += fill;
            }
            bucket_remaining -= fill;
            remaining -= fill;

            if bucket_remaining <= 0.0 {
                // Bucket complete -- record imbalance.
                let total = bucket_buy + bucket_sell;
                let imbalance = if total > 0.0 {
                    (bucket_buy - bucket_sell).abs() / total
                } else {
                    0.0
                };
                imbalances.push(imbalance);
                bucket_buy = 0.0;
                bucket_sell = 0.0;
                bucket_remaining = bucket_vol;
            }
        }
    }

    if imbalances.is_empty() {
        return 0.0;
    }

    // VPIN = mean imbalance over the last n_buckets (or all available).
    let start = imbalances.len().saturating_sub(n_buckets);
    let slice = &imbalances[start..];
    let mean: f32 = slice.iter().sum::<f32>() / slice.len() as f32;
    mean.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_buys_gives_max_vpin() {
        // Strictly increasing close -> all bars are "buys" -> imbalance = 1.0.
        let close: Vec<f32> = (0..20).map(|i| 100.0 + i as f32).collect();
        let volume = vec![100.0; 20];
        let vpin = compute_vpin(&close, &volume, 5);
        assert!((vpin - 1.0).abs() < 0.01, "vpin = {vpin}");
    }

    #[test]
    fn alternating_gives_low_vpin() {
        // Alternating up/down -> balanced -> low imbalance.
        let mut close = vec![100.0_f32; 20];
        for (i, c) in close.iter_mut().enumerate().take(20).skip(1) {
            *c = if i % 2 == 0 { 101.0 } else { 99.0 };
        }
        let volume = vec![100.0; 20];
        let vpin = compute_vpin(&close, &volume, 5);
        assert!(vpin < 0.5, "vpin = {vpin}");
    }
}
