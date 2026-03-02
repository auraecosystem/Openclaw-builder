/// Row-major f32 matrix: `data[row * n_cols + col]`.
///
/// Every daily indicator and OHLCV field is stored as one `WideMatrix` where
/// rows are trading days and columns are tickers.  Fields are private except
/// `data` which is `pub(crate)` so sibling modules (filters, signals) can
/// read the backing slice directly when bulk iteration is faster than
/// per-element `get()`.
pub struct WideMatrix {
    pub data: Vec<f32>,
    n_rows: usize,
    n_cols: usize,
}

impl WideMatrix {
    pub fn new(data: Vec<f32>, n_rows: usize, n_cols: usize) -> Self {
        debug_assert_eq!(
            data.len(),
            n_rows * n_cols,
            "WideMatrix::new: data.len()={} != {}x{}",
            data.len(),
            n_rows,
            n_cols,
        );
        Self {
            data,
            n_rows,
            n_cols,
        }
    }

    #[inline(always)]
    pub fn get(&self, row: usize, col: usize) -> f32 {
        self.data[row * self.n_cols + col]
    }

    pub fn n_rows(&self) -> usize {
        self.n_rows
    }

    pub fn n_cols(&self) -> usize {
        self.n_cols
    }

    #[allow(dead_code)]
    pub fn data_mut(&mut self) -> &mut [f32] {
        &mut self.data
    }
}

/// Boolean mask with the same (rows x cols) shape as `WideMatrix`.
///
/// Used for universe filters and entry/exit signals.
pub struct WideMask {
    pub data: Vec<bool>,
    n_rows: usize,
    n_cols: usize,
}

impl WideMask {
    /// Create a mask initialised to `false` everywhere.
    pub fn new_false(n_rows: usize, n_cols: usize) -> Self {
        Self {
            data: vec![false; n_rows * n_cols],
            n_rows,
            n_cols,
        }
    }

    #[inline(always)]
    pub fn get(&self, row: usize, col: usize) -> bool {
        self.data[row * self.n_cols + col]
    }

    #[inline(always)]
    pub fn set(&mut self, row: usize, col: usize, val: bool) {
        self.data[row * self.n_cols + col] = val;
    }

    pub fn n_rows(&self) -> usize {
        self.n_rows
    }

    pub fn n_cols(&self) -> usize {
        self.n_cols
    }
}
