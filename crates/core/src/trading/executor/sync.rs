use std::collections::HashMap;

use polars::prelude::*;

use super::super::strategy::StrategyBase;
use super::super::{Strategy, View};
use super::TaggedStep;

pub struct SyncExecutor<S> {
    pub strategy: S,
}

impl<S> SyncExecutor<S> {
    pub fn new(strategy: S) -> Self {
        Self { strategy }
    }
}

impl<S: Strategy> SyncExecutor<S> {
    pub fn run<V: View<Item = S::Input>>(
        &mut self,
        df: &DataFrame,
        view: V,
    ) -> S::Frame {
        let mut history = DataFrame::empty();
        let mut state = S::State::default();
        let mut frame = self.strategy.create_output();

        for step in view.steps(df) {
            let output = self.strategy.on_step(&step, &history, &mut state);
            self.strategy.append_output(&mut frame, output, &step);
            view.append(&mut history, &step);
        }

        frame
    }
}

impl<S: StrategyBase> SyncExecutor<S> {
    pub fn run_multi<V: View<Item = S::Input>>(
        &mut self,
        dfs: &HashMap<S::Key, DataFrame>,
        views: HashMap<S::Key, V>,
    ) -> S::Frame {
        let mut history: HashMap<S::Key, DataFrame> = HashMap::new();
        for key in views.keys() {
            history.insert(key.clone(), DataFrame::empty());
        }
        let mut state = S::State::default();
        let mut frame = self.strategy.create_output();

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
            self.strategy
                .append_output(&mut frame, output, &ts.step);
            let hist_df = history.get_mut(&ts.key).unwrap();
            views[&ts.key].append(hist_df, &ts.step);
        }

        frame
    }
}
