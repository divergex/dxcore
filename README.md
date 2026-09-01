# dxcore

**dxcore** is the data and trading library of the Divergex ecosystem.
Covers the basic networking, datastructures and pieces need to
build your own quantitative research and trading setup.

Mainly a toolkit for pulling market data, tracking portfolios,
serving networks/meshes of strategies, and talking between services.

## Main Idea

For now, the more interesting modules are:

- **Market data**: let's you fetch historical bars and live account updates from
external interfaces, such as Interactive Brokers (TWS/Gateway), and get results back.
We plan to keep expanding this to other brokers and data sources in the future.
- **Portfolios & accounts**: keep track of instruments, positions,
and account metrics in one place.
- **Strategies**: run simple strategy logic (like SMA crossovers) and factor models over your data. Also let you define your strategy code in other languages
(such as OCaml and Python), and then run in a performant env backbone.
For full strategy development and reusable strategy code, look at
[cadlag](https://github.com/divergex/cadlag).
- **Services**: register and talk between services with a small request/response layer
for now only a built-in HTTP server).
This is where the service discovery pieces that used to live in dxforge ended up.
- **Python bindings**: the same core functionality is exposed to Python via the `dxcore-pyo3` crate

and more in the future!

## Getting Started

This is a Rust workspace with two crates:

- `crates/core` — the `dxcore` library itself
- `crates/pybindings` — Python bindings (built with maturin)

Build everything with:

```bash
cargo build
```

Some functionality is behind feature flags (`ibkr` for Interactive Brokers, `strategies` for the strategy module).
Enable them if you need them:

```bash
cargo build --features ibkr,strategies
```

## Examples

A few runnable examples live in `crates/core/examples`:

```bash
cargo run --example strategy --features strategies
```

## In the Ecosystem

dxcore works alongside the other Divergex components:
[dxforge](https://github.com/divergex/dxforge) handles orchestration and containerized deployments, while **cadlag** network interfaces, input sources, and serving strategies.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
