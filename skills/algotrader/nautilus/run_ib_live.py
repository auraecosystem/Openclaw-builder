"""Connect NautilusTrader to IB Gateway and print live bar data.

Data-only node — no execution client, no orders. Validates the
NT ↔ IB Gateway connection.

Usage:
    python -m nautilus.run_ib_live
    python -m nautilus.run_ib_live --port 4001 --symbols SPY.ARCA,AAPL.NASDAQ
"""

from __future__ import annotations

import argparse

from nautilus_trader.adapters.interactive_brokers.common import IB
from nautilus_trader.adapters.interactive_brokers.config import (
    IBMarketDataTypeEnum,
    InteractiveBrokersDataClientConfig,
    InteractiveBrokersInstrumentProviderConfig,
    SymbologyMethod,
)
from nautilus_trader.adapters.interactive_brokers.factories import (
    InteractiveBrokersLiveDataClientFactory,
)
from nautilus_trader.config import (
    LiveDataEngineConfig,
    LoggingConfig,
    StrategyConfig,
    TradingNodeConfig,
)
from nautilus_trader.live.node import TradingNode
from nautilus_trader.model.data import Bar, BarType
from nautilus_trader.model.identifiers import InstrumentId
from nautilus_trader.trading.strategy import Strategy


# ---------------------------------------------------------------------------
# Minimal strategy that subscribes to bars and prints them
# ---------------------------------------------------------------------------


class PrintBarConfig(StrategyConfig, frozen=True):
    instrument_ids: list[str]
    bar_spec: str = "1-DAY-LAST"


class PrintBarStrategy(Strategy):
    """Subscribe to IB bars and print each one as it arrives."""

    def __init__(self, config: PrintBarConfig) -> None:
        super().__init__(config)

    def on_start(self) -> None:
        for iid_str in self.config.instrument_ids:
            iid = InstrumentId.from_str(iid_str)
            instrument = self.cache.instrument(iid)
            if instrument is None:
                self.log.error(f"Instrument {iid} not found in cache, skipping")
                continue

            # INTERNAL = IB computes the bars server-side
            bt = BarType.from_str(f"{iid_str}-{self.config.bar_spec}-INTERNAL")
            self.subscribe_bars(bt)
            self.log.info(f"Subscribed to {bt}")

    def on_bar(self, bar: Bar) -> None:
        self.log.info(
            f"{bar.bar_type.instrument_id} | "
            f"O={bar.open} H={bar.high} L={bar.low} C={bar.close} "
            f"V={bar.volume} | {bar.ts_event}",
        )

    def on_stop(self) -> None:
        self.log.info("Strategy stopped.")


# ---------------------------------------------------------------------------
# Node setup
# ---------------------------------------------------------------------------


def main():
    parser = argparse.ArgumentParser(description="NautilusTrader IB Gateway data node")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=4002, help="4002=Gateway paper, 4001=live")
    parser.add_argument("--client-id", type=int, default=1)
    parser.add_argument("--symbols", default="SPY.ARCA", help="Comma-separated IB simplified IDs")
    parser.add_argument("--bar-spec", default="1-DAY-LAST")
    parser.add_argument("--rth", action="store_true", help="Regular trading hours only (default: include extended)")
    parser.add_argument("--log-level", default="INFO")
    args = parser.parse_args()

    symbols = [s.strip() for s in args.symbols.split(",")]

    instrument_provider = InteractiveBrokersInstrumentProviderConfig(
        symbology_method=SymbologyMethod.IB_SIMPLIFIED,
        load_ids=frozenset(symbols),
    )

    config_node = TradingNodeConfig(
        trader_id="IB-DATA-001",
        logging=LoggingConfig(log_level=args.log_level),
        data_clients={
            IB: InteractiveBrokersDataClientConfig(
                ibg_host=args.host,
                ibg_port=args.port,
                ibg_client_id=args.client_id,
                use_regular_trading_hours=args.rth,
                market_data_type=IBMarketDataTypeEnum.DELAYED_FROZEN,
                instrument_provider=instrument_provider,
            ),
        },
        data_engine=LiveDataEngineConfig(
            time_bars_timestamp_on_close=False,
            validate_data_sequence=True,
        ),
        timeout_connection=90.0,
        timeout_reconciliation=5.0,
        timeout_portfolio=5.0,
        timeout_disconnection=5.0,
        timeout_post_stop=2.0,
    )

    node = TradingNode(config=config_node)

    strategy = PrintBarStrategy(
        config=PrintBarConfig(
            instrument_ids=symbols,
            bar_spec=args.bar_spec,
        ),
    )
    node.trader.add_strategy(strategy)

    node.add_data_client_factory(IB, InteractiveBrokersLiveDataClientFactory)
    node.build()

    try:
        node.run()
    finally:
        node.dispose()


if __name__ == "__main__":
    main()
