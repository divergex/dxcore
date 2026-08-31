use std::collections::HashMap;

use polars::prelude::*;

use super::super::strategy::StrategyBase;
use super::super::{Strategy, View};
use super::OutputRow;

pub struct AsyncExecutor<S> {
    pub strategy: S,
}

impl<S> AsyncExecutor<S> {
    pub fn new(strategy: S) -> Self {
        Self { strategy }
    }
}

impl<S: Strategy> AsyncExecutor<S> {
    pub fn run<V, St>(
        &mut self,
        stream: St,
        view: V,
    ) -> impl futures::Stream<Item = OutputRow<S::Output>> + '_
    where
        V: View<Item = S::Input> + 'static,
        St: futures::Stream<Item = S::Input> + Unpin + 'static,
        S::Input: 'static,
    {
        use std::pin::Pin;
        use std::task::{Context, Poll};
        use futures::Stream;

        struct RunStream<
            'a,
            S2: Strategy,
            V2: View<Item = S2::Input>,
            St2: Stream<Item = S2::Input> + Unpin,
        > {
            strategy: &'a mut S2,
            view: V2,
            stream: St2,
            history: DataFrame,
            state: S2::State,
            frame: S2::Frame,
        }

        impl<'a, S2, V2, St2> Stream for RunStream<'a, S2, V2, St2>
        where
            S2: Strategy,
            V2: View<Item = S2::Input>,
            St2: Stream<Item = S2::Input> + Unpin,
        {
            type Item = OutputRow<S2::Output>;

            fn poll_next(
                self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                let this = unsafe { self.get_unchecked_mut() };
                match Pin::new(&mut this.stream).poll_next(cx) {
                    Poll::Ready(Some(step)) => {
                        let output = this.strategy.on_step(
                            &step,
                            &this.history,
                            &mut this.state,
                        );
                        this.strategy
                            .append_output(&mut this.frame, output.clone(), &step);
                        this.view.append(&mut this.history, &step);
                        Poll::Ready(Some(OutputRow { output }))
                    }
                    Poll::Ready(None) => Poll::Ready(None),
                    Poll::Pending => Poll::Pending,
                }
            }
        }

        let frame = self.strategy.create_output();

        RunStream {
            strategy: &mut self.strategy,
            view,
            stream,
            history: DataFrame::empty(),
            state: S::State::default(),
            frame,
        }
    }
}

impl<S: StrategyBase> AsyncExecutor<S> {
    pub fn run_multi<V, St>(
        &mut self,
        streams: HashMap<S::Key, St>,
        views: HashMap<S::Key, V>,
    ) -> impl futures::Stream<Item = OutputRow<S::Output>> + '_
    where
        V: View<Item = S::Input> + 'static,
        St: futures::Stream<Item = S::Input> + Unpin + 'static,
        S::Input: 'static,
        S::Key: 'static,
    {
        use std::pin::Pin;
        use std::task::{Context, Poll};
        use futures::Stream;

        struct RunStream<
            'a,
            S2: StrategyBase,
            V2: View<Item = S2::Input>,
            St2: Stream<Item = S2::Input> + Unpin,
        > {
            strategy: &'a mut S2,
            views: HashMap<S2::Key, V2>,
            streams: Vec<(S2::Key, St2)>,
            history: HashMap<S2::Key, DataFrame>,
            state: S2::State,
            frame: S2::Frame,
        }

        impl<'a, S2, V2, St2> Stream for RunStream<'a, S2, V2, St2>
        where
            S2: StrategyBase,
            V2: View<Item = S2::Input>,
            St2: Stream<Item = S2::Input> + Unpin,
        {
            type Item = OutputRow<S2::Output>;

            fn poll_next(
                self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                let this = unsafe { self.get_unchecked_mut() };
                let mut all_exhausted = true;

                for (key, stream) in &mut this.streams {
                    match Pin::new(stream).poll_next(cx) {
                        Poll::Ready(Some(step)) => {
                            let output = this.strategy.on_step(
                                &step,
                                key,
                                &this.history,
                                &mut this.state,
                            );
                            this.strategy
                                .append_output(&mut this.frame, output.clone(), &step);
                            let hist_df = this
                                .history
                                .entry(key.clone())
                                .or_insert_with(DataFrame::empty);
                            this.views[key].append(hist_df, &step);
                            return Poll::Ready(Some(OutputRow { output }));
                        }
                        Poll::Ready(None) => {}
                        Poll::Pending => {
                            all_exhausted = false;
                        }
                    }
                }

                if all_exhausted {
                    Poll::Ready(None)
                } else {
                    Poll::Pending
                }
            }
        }

        let frame = self.strategy.create_output();
        let mut history = HashMap::new();
        for key in views.keys() {
            history.insert(key.clone(), DataFrame::empty());
        }

        RunStream {
            strategy: &mut self.strategy,
            views,
            streams: streams.into_iter().collect(),
            history,
            state: S::State::default(),
            frame,
        }
    }
}
