use std::f64::consts::PI;
use std::sync::Arc;

use polars::prelude::*;

use dxcore::interface::internal::{
    http_on_step, registry_guard, HttpAccessor, InterfaceFactory, MethodKind, ServiceSpec,
    StepArgs, StrategyInterface,
};
use dxcore::network::mesh::Protocol;
use dxcore::network::servers::{HttpServer, ServerHandle};
use dxcore::network::services::FunctionalService;
use dxcore::trading::{Signal, Strategy};

#[derive(Default)]
struct CrossState {
    prev_short: Option<f64>,
    prev_long: Option<f64>,
    shares: f64,
}

struct Cross {
    short: usize,
    long: usize,
}

impl Strategy for Cross {
    type Input = (i32, DataFrame);
    type State = CrossState;
    type Output = Option<Signal>;

    fn on_step(
        &self,
        (_date, step): &(i32, DataFrame),
        history: &DataFrame,
        state: &mut CrossState,
    ) -> Option<Signal> {
        let price = step.column("price").unwrap().f64().unwrap().get(0).unwrap();
        let short = sma(history, "price", self.short);
        let long = sma(history, "price", self.long);
        let before = state.shares;

        match (state.prev_short, state.prev_long, short, long) {
            (Some(ps), Some(pl), Some(s), Some(l)) => {
                if ps <= pl && s > l {
                    state.shares = (1_000.0 / price).trunc();
                } else if ps >= pl && s < l {
                    state.shares = 0.0;
                }
            }
            _ => state.shares = (1_000.0 / price).trunc(),
        }

        state.prev_short = short;
        state.prev_long = long;

        (state.shares != before).then(|| Signal {
            symbol: Some("DEMO".into()),
            shares: state.shares,
        })
    }
}

struct Runner {
    strategy: Cross,
    state: CrossState,
}

fn sma(history: &DataFrame, col: &str, window: usize) -> Option<f64> {
    let series = history.column(col).ok()?.f64().ok()?;
    let vals: Vec<f64> = series.into_iter().flatten().collect();
    if vals.len() < window {
        return None;
    }
    Some(vals.iter().rev().take(window).sum::<f64>() / window as f64)
}

fn demo_steps() -> Vec<(i32, DataFrame)> {
    (0..40)
        .map(|i| {
            let price = (100.0 + 10.0 * ((i as f64) * 0.5 * PI / 2.0).sin()).round();
            let frame = DataFrame::new(vec![Column::new("price".into(), vec![price])]).unwrap();
            (19000 + i as i32, frame)
        })
        .collect()
}

fn append(history: &mut DataFrame, step: &DataFrame) {
    if history.width() == 0 {
        *history = step.clone();
    } else {
        history.vstack_mut(step).unwrap();
    }
}

fn spawn_server(strategy: Cross) -> (String, ServerHandle) {
    let service = FunctionalService::new(
        "strategy",
        Runner {
            strategy,
            state: CrossState::default(),
        },
    )
    .with_set("on_step", |runner: &mut Runner, args: StepArgs| {
        let (step, history) = args.into_parts();
        Ok(runner.strategy.on_step(&step, &history, &mut runner.state))
    });

    let server = HttpServer::bind("127.0.0.1:0", Arc::new(service)).unwrap();
    let addr = server.addr();
    let handle = server.spawn();
    (format!("http://{addr}"), handle)
}

fn connect(url: &str) -> StrategyInterface<Option<Signal>> {
    let interface = InterfaceFactory::new("strategy")
        .accessor(Protocol::Http, HttpAccessor::new(url))
        .set("on_step", Protocol::Http, http_on_step)
        .build()
        .expect("build the strategy interface");
    StrategyInterface::new(interface)
}

#[test]
fn interface_matches_a_local_strategy() {
    let (base, handle) = spawn_server(Cross { short: 3, long: 6 });
    let remote = connect(&base);
    let local = Cross { short: 3, long: 6 };

    let mut local_state = CrossState::default();
    let mut remote_state = ();
    let mut history = DataFrame::empty();
    let mut signals = 0;

    for step in demo_steps() {
        let expected = local.on_step(&step, &history, &mut local_state);
        let actual = remote.on_step(&step, &history, &mut remote_state);
        assert_eq!(actual, expected);

        signals += expected.is_some() as usize;
        append(&mut history, &step.1);
    }

    assert!(signals > 0, "demo data must cross over");
    assert!(remote.take_error().is_none());

    handle.stop().unwrap();
}

#[test]
fn interface_stashes_call_error() {
    // Nothing listens on port 1: the step returns the default output and
    // surfaces the failure through `take_error`.
    let remote = connect("http://127.0.0.1:1");
    let step = &demo_steps()[0];

    let output = remote.on_step(step, &DataFrame::empty(), &mut ());

    assert_eq!(output, None);
    assert!(remote.take_error().is_some());
}

#[test]
fn build_rejects_a_method_the_service_does_not_declare() {
    let err = InterfaceFactory::new("strategy")
        .accessor(Protocol::Http, HttpAccessor::new("http://127.0.0.1:1"))
        .set("nope", Protocol::Http, http_on_step)
        .build()
        .unwrap_err();

    assert!(err.to_string().contains("no method nope"), "{err}");
}

#[test]
fn build_rejects_an_unregistered_service() {
    let err = InterfaceFactory::new("nope")
        .accessor(Protocol::Http, HttpAccessor::new("http://127.0.0.1:1"))
        .build()
        .unwrap_err();

    assert!(err.to_string().contains("not registered"), "{err}");
}

fn readable_interface(
    factory: InterfaceFactory,
) -> Result<dxcore::interface::internal::Interface, dxcore::Error> {
    factory
        .accessor(Protocol::Http, HttpAccessor::new("http://127.0.0.1:1"))
        .get("read", Protocol::Http, |_: &HttpAccessor, args: u32| {
            Ok::<_, dxcore::Error>(args + 1)
        })
        .build()
}

#[test]
fn a_user_registered_service_spec_gives_access() {
    registry_guard().register(ServiceSpec::new("thing").with_get("read", &[Protocol::Http]));

    {
        let registry = registry_guard();
        let spec = registry.get("thing").unwrap().method("read").unwrap();
        assert_eq!(spec.kind(), MethodKind::Get);
        assert!(spec.accepts(Protocol::Http));
    }

    let interface = readable_interface(InterfaceFactory::new("thing")).unwrap();
    assert_eq!(interface.service(), "thing");
}

#[test]
fn build_rejects_a_kind_mismatch() {
    registry_guard().register(ServiceSpec::new("readonly").with_get("read", &[Protocol::Http]));

    let err = InterfaceFactory::new("readonly")
        .accessor(Protocol::Http, HttpAccessor::new("http://127.0.0.1:1"))
        .set("read", Protocol::Http, |_: &HttpAccessor, args: u32| {
            Ok::<_, dxcore::Error>(args)
        })
        .build()
        .unwrap_err();

    assert!(err.to_string().contains("registered as a Set"), "{err}");
}

#[test]
fn build_rejects_a_protocol_without_an_accessor() {
    registry_guard().register(ServiceSpec::new("noaccess").with_get("read", &[Protocol::Http]));

    let err = InterfaceFactory::new("noaccess")
        .get("read", Protocol::Http, |_: &HttpAccessor, args: u32| {
            Ok::<_, dxcore::Error>(args)
        })
        .build()
        .unwrap_err();

    assert!(err.to_string().contains("has no accessor"), "{err}");
}
