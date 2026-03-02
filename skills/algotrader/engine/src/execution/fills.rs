//! Fill resolution for entry orders and intraday stop checks.
//!
//! Three fill modes map to the three strategy families:
//! - `NextDayOpen` -- breakout and parabolic: signal on day N, fill at open of N+1.
//! - `SameDayOpen` -- EP (episodic pivot): signal already shifted to entry day.
//! - `DualTimeframe` -- daily signal + 5m bar confirmation on day N+1.

use engine_data::DataStore;
use engine_types::Direction;

#[derive(Clone, Copy, Debug)]
pub enum FillMode {
    /// Fill at next day's open. Used by breakout and parabolic strategies.
    NextDayOpen,
    /// Signal already shifted to entry day; fill at that day's open.
    /// Used by EP where the signal module places signals on the fill day.
    SameDayOpen,
    /// Daily signal triggers a search for 5m confirmation on the next day.
    /// Falls back to None if no 5m breakout is confirmed.
    DualTimeframe,
}

/// Resolve entry fill price and actual entry row.
///
/// Returns `None` if the fill is invalid (NaN price, out-of-range row, or
/// no 5m confirmation for DualTimeframe mode).
pub fn resolve_fill(
    store: &DataStore,
    signal_row: usize,
    col: usize,
    mode: FillMode,
    end_row: usize,
) -> Option<(f32, usize)> {
    match mode {
        FillMode::NextDayOpen => {
            let next = signal_row + 1;
            if next >= end_row {
                return None;
            }
            let fp = store.open().get(next, col);
            if fp.is_nan() || fp <= 0.0 {
                return None;
            }
            Some((fp, next))
        }

        FillMode::SameDayOpen => {
            // EP signals are pre-shifted: the signal row IS the fill day
            let fp = store.open().get(signal_row, col);
            if fp.is_nan() || fp <= 0.0 {
                return None;
            }
            Some((fp, signal_row))
        }

        FillMode::DualTimeframe => find_5m_entry(store, signal_row, col),
    }
}

/// Check the 5m bars within a daily bar for an intraday stop hit.
///
/// Scans the 5m bars mapped to `daily_row`. Returns `Some(exit_price)` at
/// the first 5m close that breaches the stop level, or `None` if the stop
/// held all day.
pub fn check_5m_stop(
    store: &DataStore,
    daily_row: usize,
    col: usize,
    stop_price: f32,
    direction: Direction,
) -> Option<f32> {
    let intraday = store.intraday.as_ref()?;

    if daily_row >= intraday.day_mapping.len() {
        return None;
    }
    let (start, end) = intraday.day_mapping[daily_row];

    // Intraday close is stored in the Close slot of intraday matrices
    let close_5m = &intraday.matrices[engine_data::Indicator::Close as usize];

    for r5 in start..end {
        let c5 = close_5m.get(r5, col);
        if c5.is_nan() {
            continue;
        }
        match direction {
            Direction::Long => {
                if c5 <= stop_price {
                    return Some(c5);
                }
            }
            Direction::Short => {
                if c5 >= stop_price {
                    return Some(c5);
                }
            }
        }
    }

    None
}

/// Try to find a 5m entry on the next trading day after a daily signal.
///
/// Scans next-day 5m bars for the first close above the consolidation high.
/// Returns `(fill_price, daily_row + 1)` if confirmed, `None` otherwise.
fn find_5m_entry(store: &DataStore, daily_row: usize, col: usize) -> Option<(f32, usize)> {
    let intraday = store.intraday.as_ref()?;

    let next_day = daily_row + 1;
    if next_day >= intraday.day_mapping.len() {
        return None;
    }
    let (start, end) = intraday.day_mapping[next_day];
    if start >= end {
        return None;
    }

    let close_5m = &intraday.matrices[engine_data::Indicator::Close as usize];
    let volume_5m = &intraday.matrices[engine_data::Indicator::Volume as usize];

    // Read the consolidation high from daily data
    let consol_high = store
        .get(engine_data::Indicator::ConsolHigh)
        .get(daily_row, col);
    if consol_high.is_nan() {
        return None;
    }

    // Compute average volume over the day's 5m bars for threshold
    let mut vol_sum = 0.0f32;
    let mut vol_count = 0usize;
    for r5 in start..end {
        let v = volume_5m.get(r5, col);
        if !v.is_nan() && v > 0.0 {
            vol_sum += v;
            vol_count += 1;
        }
    }
    let avg_vol = if vol_count > 0 {
        vol_sum / vol_count as f32
    } else {
        0.0
    };

    // Scan for first 5m bar where close breaks above consolidation high.
    // Volume confirmation (1.5x avg) is preferred but not required.
    for r5 in start..end {
        let c5 = close_5m.get(r5, col);
        if c5.is_nan() {
            continue;
        }
        if c5 > consol_high {
            // Volume spike check is informational -- we take the breakout regardless
            let _v5 = volume_5m.get(r5, col);
            let _has_volume = avg_vol > 0.0 && _v5 > avg_vol * 1.5;
            return Some((c5, next_day));
        }
    }

    None
}
