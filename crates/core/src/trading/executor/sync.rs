use std::collections::HashMap;

use polars::prelude::*;

use super::super::strategy::StrategyBase;
use super::super::{OrderEngine, OrderError, Strategy, View};
use super::TaggedStep;

/// Runs a strategy against a frame through an [`OrderEngine`].
pub struct SyncExecutor<E, S> {
    pub engine: E,
    pub strategy: S,
}

impl<E, S> SyncExecutor<E, S> {
    pub fn new(engine: E, strategy: S) -> Self {
        Self { engine, strategy }
    }
}

impl<E, S: Strategy> SyncExecutor<E, S> {
    /// Feeds `strategy` every step `view` produces, in order, and returns the
    /// orders the engine translated. Steps the engine turns into no order leave
    /// the frame untouched.
    pub fn run<V>(&mut self, df: &DataFrame, view: V) -> Result<E::Frame, OrderError>
    where
        V: View<Item = S::Input>,
        E: OrderEngine<S::Output, S::Input>,
    {
        let mut history = DataFrame::empty();
        let mut state = S::State::default();
        let mut engine_state = E::State::default();
        let mut frame = self.engine.create_output();

        for step in view.steps(df) {
            let output = self.strategy.on_step(&step, &history, &mut state);
            let date = view.step_ord_key(&step);
            let order = self
                .engine
                .transform(output, &step, date, &mut engine_state)?;
            if let Some(order) = order {
                self.engine.append_output(&mut frame, order, &step);
            }
            view.append(&mut history, &step);
        }

        Ok(frame)
    }
}

impl<E, S: StrategyBase> SyncExecutor<E, S> {
    /// Multi-key variant of [`SyncExecutor::run`]: steps from all views are
    /// interleaved by [`View::step_ord_key`].
    pub fn run_multi<V>(
        &mut self,
        dfs: &HashMap<S::Key, DataFrame>,
        views: HashMap<S::Key, V>,
    ) -> Result<E::Frame, OrderError>
    where
        V: View<Item = S::Input>,
        E: OrderEngine<S::Output, S::Input>,
    {
        let mut history: HashMap<S::Key, DataFrame> = HashMap::new();
        for key in views.keys() {
            history.insert(key.clone(), DataFrame::empty());
        }
        let mut state = S::State::default();
        let mut engine_state = E::State::default();
        let mut frame = self.engine.create_output();

        let mut tagged: Vec<TaggedStep<S::Key, S::Input>> = Vec::new();
        for (key, view) in &views {
            let df = dfs
                .get(key)
                .expect("run_multi: missing DataFrame for key");
            for step in view.steps(df) {
                let ord = view.step_ord_key(&step);
                tagged.push(TaggedStep {
                    key: key.clone(),
                    step,
                    ord,
                });
            }
        }

        tagged.sort_by(|a, b| match (a.ord, b.ord) {
            (Some(oa), Some(ob)) => oa.cmp(&ob),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });

        for ts in tagged {
            let output = self
                .strategy
                .on_step(&ts.step, &ts.key, &history, &mut state);
            let order = self
                .engine
                .transform(output, &ts.step, ts.ord, &mut engine_state)?;
            if let Some(order) = order {
                self.engine.append_output(&mut frame, order, &ts.step);
            }
            let hist_df = history.get_mut(&ts.key).unwrap();
            views[&ts.key].append(hist_df, &ts.step);
        }

        Ok(frame)
    }
}
