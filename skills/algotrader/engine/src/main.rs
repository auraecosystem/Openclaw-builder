//! Slim CLI dispatch: parse args, load data, delegate to library functions.

mod cli;

use std::path::Path;
use std::time::Instant;

use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let args = cli::Cli::parse();
    let cfg = algotrader_engine::trading_config::load_trading_config()?;
    algotrader_engine::trading_config::set_trading_config(cfg);
    let data_dir = algotrader_engine::trading_config::dataset_dir_for_profile(&args.profile)?;

    // Build base params early so DataConfig is available for data loading.
    // The --crypto flag and --config file both affect DataConfig.
    let base_params = args.to_params();

    eprintln!("Loading data from {}...", data_dir.display());
    let t0 = Instant::now();
    let store = algotrader_engine::load_data_store(&data_dir, &base_params.data)?;
    eprintln!("  Loaded in {:.2}s", t0.elapsed().as_secs_f64());

    if let Some(portfolio_dir) = &args.portfolio {
        let t1 = Instant::now();
        let result = algotrader_engine::portfolio::run_portfolio(
            &store,
            std::path::Path::new(portfolio_dir),
        )?;
        eprintln!(
            "  Portfolio complete in {:.2}s ({} strategies)",
            t1.elapsed().as_secs_f64(),
            result.strategies.len(),
        );
        let json = serde_json::to_string_pretty(&result)?;
        println!("{}", json);
    } else if args.evolve {
        use algotrader_engine::evolution::fitness::FitnessMetric;
        use algotrader_engine::evolution::{Algorithm, EvolutionConfig};

        let config = EvolutionConfig {
            algorithm: Algorithm::parse(&args.algo),
            generations: args.generations,
            pop_size: args.pop_size_evo,
            fitness_metric: FitnessMetric::parse(&args.fitness),
            initial_sigma: args.sigma,
            evolve_signals: args.evolve_signals,
            evolve_patterns: args.evolve_patterns,
            crypto: args.crypto,
            seed: args.seed,
        };

        let mut base = base_params.clone();
        if let Some(mt) = args.min_trades {
            base.fitness.min_trades_gate = mt;
            // Scale confidence ramp to reach 1.0 at 2x the gate
            base.fitness.confidence_range = mt as f64;
        }

        let t1 = Instant::now();
        if args.wf_folds > 0 {
            let result = algotrader_engine::evolution::evolve_walk_forward(
                &store,
                &base,
                &config,
                args.wf_folds,
            );
            eprintln!(
                "  Walk-forward evolution complete in {:.2}s",
                t1.elapsed().as_secs_f64()
            );
            let json = serde_json::to_string_pretty(&result)?;
            println!("{}", json);
        } else {
            let result = algotrader_engine::evolution::evolve(&store, &base, &config);
            eprintln!(
                "  Evolution complete in {:.2}s ({} generations)",
                t1.elapsed().as_secs_f64(),
                result.generations_run,
            );
            let json = serde_json::to_string_pretty(&result)?;
            println!("{}", json);
        }
    } else if args.serve {
        algotrader_engine::server::serve(&store, args.port)?;
    } else if let Some(batch_path) = &args.batch {
        let t1 = Instant::now();
        let reports = algotrader_engine::run_batch(&store, Path::new(batch_path))?;
        let elapsed = t1.elapsed().as_secs_f64();
        eprintln!(
            "  Batch complete: {} runs in {:.2}s ({:.1}ms/run)",
            reports.len(),
            elapsed,
            elapsed * 1000.0 / reports.len().max(1) as f64,
        );
        let json = serde_json::to_string(&reports)?;
        println!("{}", json);
    } else if let Some(ref compare_arg) = args.compare {
        let params = base_params.clone();
        let t1 = Instant::now();

        if compare_arg == "all" {
            let results = algotrader_engine::compare::compare_all(&store, &params);
            eprintln!("  Comparison complete in {:.2}s ({} pairs)", t1.elapsed().as_secs_f64(), results.len());
            let json = serde_json::to_string_pretty(&results)?;
            println!("{}", json);
        } else {
            // Parse "hardcoded:dynamic" format
            let parts: Vec<&str> = compare_arg.split(':').collect();
            if parts.len() != 2 {
                anyhow::bail!("--compare expects 'all' or 'hardcoded:dynamic' (e.g. 'ep:ep_dynamic')");
            }
            let result = algotrader_engine::compare::compare_strategies(&store, &params, parts[0], parts[1])?;
            eprintln!("  Comparison complete in {:.2}s", t1.elapsed().as_secs_f64());
            let json = serde_json::to_string_pretty(&result)?;
            println!("{}", json);
        }
    } else if let Some(csv_path) = &args.dump_trades {
        let params = base_params.clone();
        let t1 = Instant::now();
        let (report, trades) = algotrader_engine::run_single_with_trades(&store, &params);
        eprintln!(
            "  Backtest complete in {:.2}ms",
            t1.elapsed().as_secs_f64() * 1000.0
        );
        write_trades_csv(csv_path, &trades, &store.axes)?;
        eprintln!("  Dumped {} trades to {}", trades.len(), csv_path);
        let json = serde_json::to_string_pretty(&report)?;
        println!("{}", json);
    } else {
        let params = base_params;
        let t1 = Instant::now();
        let report = algotrader_engine::run_single(&store, &params);
        eprintln!(
            "  Backtest complete in {:.2}ms",
            t1.elapsed().as_secs_f64() * 1000.0
        );
        let json = serde_json::to_string_pretty(&report)?;
        println!("{}", json);
    }

    Ok(())
}

/// Write trades to CSV for cross-engine validation in NautilusTrader.
fn write_trades_csv(
    path: &str,
    trades: &[algotrader_engine::types::Trade],
    axes: &algotrader_engine::Axes,
) -> Result<()> {
    use std::io::Write;
    let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "date,ticker,direction,stop_price")?;
    for t in trades {
        let days = axes.dates[t.entry_row];
        let date = epoch + chrono::Duration::days(days as i64);
        let ticker = &axes.tickers[t.ticker_col];
        let dir = match t.direction {
            algotrader_engine::types::Direction::Long => "long",
            algotrader_engine::types::Direction::Short => "short",
        };
        writeln!(f, "{},{},{},{:.8}", date, ticker, dir, t.initial_stop)?;
    }
    Ok(())
}
