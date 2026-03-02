//! TCP server: load data once, accept batch requests forever.
//!
//! Protocol: client sends one line of JSON (array of Params), server replies
//! with one line of JSON (array of Reports). Progress messages are interleaved
//! as single-line JSON objects with `{"progress": N, "total": M}`.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use rayon::prelude::*;

use crate::analysis::Report;
use crate::data::DataStore;
use crate::types::Params;

/// Start a TCP server that accepts batch backtest requests.
///
/// Each connection can send multiple lines, each a JSON array of `Params`.
/// The server responds per-line with progress updates followed by the final
/// JSON array of `Report` values.
pub fn serve(store: &DataStore, port: u16) -> Result<()> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr)?;
    eprintln!("  Serving on {} (data hot, send JSON batch per line)", addr);

    for stream in listener.incoming() {
        let stream = stream?;
        let peer = stream.peer_addr().ok();
        eprintln!("  Client connected: {:?}", peer);

        let reader = BufReader::new(stream.try_clone()?);
        let mut writer = stream;

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }

            let t1 = Instant::now();
            let param_sets: Vec<Params> = match serde_json::from_str(&line) {
                Ok(p) => p,
                Err(e) => {
                    let err = serde_json::json!({"error": format!("{}", e)});
                    let _ = writeln!(writer, "{}", err);
                    continue;
                }
            };

            let total = param_sets.len();
            let done = Arc::new(AtomicUsize::new(0));

            // Progress reporter thread: send periodic updates over the socket
            let done2 = Arc::clone(&done);
            let mut writer2 = writer.try_clone()?;
            let progress_handle = std::thread::spawn(move || {
                let mut last_reported = 0;
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    let current = done2.load(Ordering::Relaxed);
                    if current != last_reported {
                        let msg = serde_json::json!({"progress": current, "total": total});
                        let _ = writeln!(writer2, "{}", msg);
                        let _ = writer2.flush();
                        last_reported = current;
                    }
                    if current >= total {
                        break;
                    }
                }
            });

            // Run backtests in parallel, incrementing counter on each completion
            let reports: Vec<Report> = param_sets
                .par_iter()
                .map(|p| {
                    let r = crate::run_single(store, p);
                    done.fetch_add(1, Ordering::Relaxed);
                    r
                })
                .collect();

            let _ = progress_handle.join();

            let elapsed = t1.elapsed().as_secs_f64();
            eprintln!(
                "  Batch: {} runs in {:.2}s ({:.1}ms/run)",
                reports.len(),
                elapsed,
                elapsed * 1000.0 / reports.len().max(1) as f64,
            );

            let json = serde_json::to_string(&reports)?;
            writeln!(writer, "{}", json)?;
            writer.flush()?;
        }

        eprintln!("  Client disconnected: {:?}", peer);
    }

    Ok(())
}
