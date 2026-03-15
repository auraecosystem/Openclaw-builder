use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result};
use serde::Deserialize;
use toml::Value;


static TRADING_CONFIG: OnceLock<TradingConfig> = OnceLock::new();
const PLACEHOLDER: &str = "__SET_IN_trading.local.toml__";

#[derive(Clone, Debug, Deserialize)]
pub struct TradingConfig {
    pub paths: PathsConfig,
    pub datasets: HashMap<String, String>,
    pub artifacts: HashMap<String, String>,
    pub profiles: HashMap<String, ProfileConfig>,
    pub integration: HashMap<String, String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PathsConfig {
    pub repo_root: String,
    pub data_root: String,
    pub nautilus_repo_root: String,
    pub lab_notebook_path: String,
    pub strategies_dir: String,
    pub engine_binary: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProfileConfig {
    pub dataset: String,
    pub timeframe: String,
    pub venue: Option<String>,
    pub bar_spec: Option<String>,
    pub universe: Option<String>,
    pub raw_dir: Option<String>,
    pub ohlcv: Option<String>,
    pub catalog: Option<String>,
}

fn merge_toml(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Table(base_map), Value::Table(overlay_map)) => {
            for (key, value) in overlay_map {
                match base_map.get_mut(&key) {
                    Some(existing) => merge_toml(existing, value),
                    None => {
                        base_map.insert(key, value);
                    }
                }
            }
        }
        (slot, value) => *slot = value,
    }
}

fn read_toml(path: &Path) -> Result<Value> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read TOML config {}", path.display()))?;
    let value: Value = toml::from_str(&content)
        .with_context(|| format!("Failed to parse TOML config {}", path.display()))?;
    Ok(value)
}

fn require_real_path(value: &str, field: &str) -> Result<()> {
    if value.is_empty() || value == PLACEHOLDER {
        anyhow::bail!("Missing required algotrader config value: {field}");
    }
    Ok(())
}

fn validate(cfg: &TradingConfig) -> Result<()> {
    require_real_path(&cfg.paths.repo_root, "paths.repo_root")?;
    require_real_path(&cfg.paths.data_root, "paths.data_root")?;
    require_real_path(&cfg.paths.nautilus_repo_root, "paths.nautilus_repo_root")?;
    require_real_path(&cfg.paths.lab_notebook_path, "paths.lab_notebook_path")?;
    require_real_path(&cfg.paths.strategies_dir, "paths.strategies_dir")?;
    require_real_path(&cfg.paths.engine_binary, "paths.engine_binary")?;
    for (key, value) in &cfg.datasets {
        require_real_path(value, &format!("datasets.{key}"))?;
    }
    for (key, value) in &cfg.artifacts {
        require_real_path(value, &format!("artifacts.{key}"))?;
    }
    for (name, profile) in &cfg.profiles {
        if !cfg.datasets.contains_key(&profile.dataset) {
            anyhow::bail!(
                "Profile '{name}' references unknown dataset '{}'",
                profile.dataset
            );
        }
        if let Some(value) = &profile.ohlcv {
            require_real_path(value, &format!("profiles.{name}.ohlcv"))?;
        }
    }
    Ok(())
}

pub fn default_config_paths() -> (PathBuf, PathBuf) {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("engine has parent directory")
        .to_path_buf();
    (
        repo_root.join("config").join("trading.defaults.toml"),
        repo_root.join("config").join("trading.local.toml"),
    )
}

pub fn load_trading_config() -> Result<TradingConfig> {
    let (defaults_path, local_path) = default_config_paths();
    let mut merged = read_toml(&defaults_path)?;
    if !local_path.exists() {
        anyhow::bail!(
            "Missing local algotrader config at {}",
            local_path.display()
        );
    }
    merge_toml(&mut merged, read_toml(&local_path)?);
    let cfg: TradingConfig = merged
        .try_into()
        .context("Failed to deserialize algotrader trading config")?;
    validate(&cfg)?;
    Ok(cfg)
}

pub fn set_trading_config(cfg: TradingConfig) {
    let _ = TRADING_CONFIG.set(cfg);
}

pub fn trading_config() -> Option<&'static TradingConfig> {
    TRADING_CONFIG.get()
}

pub fn dataset_dir_for_profile(profile: &str) -> Result<PathBuf> {
    let cfg = trading_config().context("Trading config has not been initialized")?;
    let profile_cfg = cfg
        .profiles
        .get(profile)
        .with_context(|| format!("Unknown algotrader profile '{profile}'"))?;
    let dataset = cfg
        .datasets
        .get(&profile_cfg.dataset)
        .with_context(|| format!("Profile '{profile}' points at unknown dataset '{}'", profile_cfg.dataset))?;
    Ok(PathBuf::from(dataset))
}

pub fn strategies_dir() -> Option<PathBuf> {
    trading_config().map(|cfg| PathBuf::from(&cfg.paths.strategies_dir))
}
