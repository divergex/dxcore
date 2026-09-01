"""Type stubs for the `dxcore` package.

Hand-written; keep in sync with `crates/pybindings/src`. The compiled
extension module carries no type information, so type checkers resolve
the package namespace through these files.
"""

from .core import AccountMetric, Instrument, InstrumentStore, Portfolio
from .interface import (
    Contract,
    FmpBalanceSheet,
    FmpClient,
    FmpIncomeStatement,
    FmpProfile,
    GuardianArticle,
    GuardianArticleBody,
    GuardianBlock,
    GuardianClient,
    IbkrInterface,
    XbrlClient,
    XbrlFiling,
)
from .network import (
    Endpoint,
    HttpServer,
    MeshService,
    Protocol,
    Registration,
    ServerHandle,
    ServiceError,
)
from .trading import AsyncExecutor, DailyView, Executor, RunIterator

__all__ = [
    "AccountMetric",
    "AsyncExecutor",
    "Contract",
    "DailyView",
    "Endpoint",
    "Executor",
    "FmpBalanceSheet",
    "FmpClient",
    "FmpIncomeStatement",
    "FmpProfile",
    "GuardianArticle",
    "GuardianArticleBody",
    "GuardianBlock",
    "GuardianClient",
    "HttpServer",
    "IbkrInterface",
    "Instrument",
    "InstrumentStore",
    "MeshService",
    "Portfolio",
    "Protocol",
    "Registration",
    "RunIterator",
    "ServerHandle",
    "ServiceError",
    "XbrlClient",
    "XbrlFiling",
]
