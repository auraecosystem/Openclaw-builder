from __future__ import annotations

from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
from typing import Any
import tomllib


_CONFIG_DIR = Path(__file__).resolve().parent / "config"
_DEFAULTS_PATH = _CONFIG_DIR / "trading.defaults.toml"
_LOCAL_PATH = _CONFIG_DIR / "trading.local.toml"
_PLACEHOLDER = "__SET_IN_trading.local.toml__"


def _merge_dicts(base: dict[str, Any], override: dict[str, Any]) -> dict[str, Any]:
    merged = dict(base)
    for key, value in override.items():
        if isinstance(value, dict) and isinstance(merged.get(key), dict):
            merged[key] = _merge_dicts(merged[key], value)
        else:
            merged[key] = value
    return merged


def _load_toml(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {}
    with path.open("rb") as fh:
        return tomllib.load(fh)


def _coerce_path(value: str, field: str) -> Path:
    if not value or value == _PLACEHOLDER:
        raise RuntimeError(f"Missing required algotrader config path: {field}")
    return Path(value).expanduser().resolve()


@dataclass(frozen=True)
class Profile:
    name: str
    dataset: str
    timeframe: str
    venue: str | None
    bar_spec: str | None
    universe: Path | None
    raw_dir: Path | None
    ohlcv: Path | None
    catalog: Path | None


@dataclass(frozen=True)
class TradingConfig:
    repo_root: Path
    data_root: Path
    nautilus_repo_root: Path
    lab_notebook_path: Path
    strategies_dir: Path
    engine_binary: Path
    datasets: dict[str, Path]
    artifacts: dict[str, Path]
    profiles: dict[str, Profile]
    integration: dict[str, str]

    def profile(self, name: str) -> Profile:
        try:
            return self.profiles[name]
        except KeyError as exc:
            known = ", ".join(sorted(self.profiles))
            raise RuntimeError(f"Unknown algotrader profile '{name}'. Known profiles: {known}") from exc


@lru_cache(maxsize=1)
def load_trading_config() -> TradingConfig:
    if not _LOCAL_PATH.exists():
        raise RuntimeError(
            f"Missing local algotrader config at {_LOCAL_PATH}. "
            f"Copy {_DEFAULTS_PATH.name} to {_LOCAL_PATH.name} and fill in real paths."
        )

    merged = _merge_dicts(_load_toml(_DEFAULTS_PATH), _load_toml(_LOCAL_PATH))
    paths = merged.get("paths", {})
    datasets = merged.get("datasets", {})
    artifacts = merged.get("artifacts", {})
    profiles_raw = merged.get("profiles", {})
    integration = merged.get("integration", {})

    resolved_datasets = {
        key: _coerce_path(value, f"datasets.{key}")
        for key, value in datasets.items()
    }
    resolved_artifacts = {
        key: _coerce_path(value, f"artifacts.{key}")
        for key, value in artifacts.items()
    }

    profiles: dict[str, Profile] = {}
    for name, profile in profiles_raw.items():
        profiles[name] = Profile(
            name=name,
            dataset=profile["dataset"],
            timeframe=profile["timeframe"],
            venue=profile.get("venue"),
            bar_spec=profile.get("bar_spec"),
            universe=_coerce_path(profile["universe"], f"profiles.{name}.universe") if profile.get("universe") else None,
            raw_dir=_coerce_path(profile["raw_dir"], f"profiles.{name}.raw_dir") if profile.get("raw_dir") else None,
            ohlcv=_coerce_path(profile["ohlcv"], f"profiles.{name}.ohlcv") if profile.get("ohlcv") else None,
            catalog=_coerce_path(profile["catalog"], f"profiles.{name}.catalog") if profile.get("catalog") else None,
        )

    return TradingConfig(
        repo_root=_coerce_path(paths["repo_root"], "paths.repo_root"),
        data_root=_coerce_path(paths["data_root"], "paths.data_root"),
        nautilus_repo_root=_coerce_path(paths["nautilus_repo_root"], "paths.nautilus_repo_root"),
        lab_notebook_path=_coerce_path(paths["lab_notebook_path"], "paths.lab_notebook_path"),
        strategies_dir=_coerce_path(paths["strategies_dir"], "paths.strategies_dir"),
        engine_binary=_coerce_path(paths["engine_binary"], "paths.engine_binary"),
        datasets=resolved_datasets,
        artifacts=resolved_artifacts,
        profiles=profiles,
        integration=dict(integration),
    )
