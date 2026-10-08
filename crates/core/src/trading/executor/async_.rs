use std::collections::HashMap;

use futures::Stream;
use polars::prelude::*;

use super::super::strategy::StrategyBase;
use super::super::{OrderEngine, OrderError, Strategy, View};
use super::OutputRow;

/// Runs a strategy over a stream of steps through an [`OrderEngine`].
pub struct AsyncExecutor<E, S> {
    pub engine: E,
    pub strategy: S,
}

impl<E, S> AsyncExecutor<E, S> {
    pub fn new(engine: E, strategy: S) -> Self {
        Self { engine, strategy }
    }
}

impl<E, S: Strategy> AsyncExecutor<E, S> {
    /// Yields one `Ok` per order the engine produced, in step order; steps that
    /// produce no order are consumed silently. A failing translation is yielded
    /// as `Err` and ends the stream for the caller to drop.
    pub fn run<V, St>(
        &mut self,
        stream: St,
        view: V,
    ) -> impl Stream<Item = Result<OutputRow<E::Order>, OrderError>> + '_
    where
        V: View<Item = S::Input> + 'static,
        St: Stream<Item = S::Input> + Unpin + 'static,
        S::Input: 'static,
        E: OrderEngine<S::Output, S::Input>,
    {
        use std::pin::Pin;
        use std::task::{Context, Poll};

        struct RunStream<'a, E2, S2, V2, St2>
        where
            S2: Strategy,
            V2: View<Item = S2::Input>,
            St2: Stream<Item = S2::Input> + Unpin,
            E2: OrderEngine<S2::Output, S2::Input>,
        {
            engine: &'a E2,
            strategy: &'a S2,
            view: V2,
            stream: St2,
            history: DataFrame,
            state: S2::State,
            engine_state: E2::State,
        }

        impl<'a, E2, S2, V2, St2> Stream for RunStream<'a, E2, S2, V2, St2>
        where
            S2: Strategy,
            V2: View<Item = S2::Input>,
            St2: Stream<Item = S2::Input> + Unpin,
            E2: OrderEngine<S2::Output, S2::Input>,
        {
            type Item = Result<OutputRow<E2::Order>, OrderError>;

            fn poll_next(
                self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                let this = unsafe { self.get_unchecked_mut() };
                loop {
                    match Pin::new(&mut this.stream).poll_next(cx) {
                        Poll::Ready(Some(step)) => {
                            let output =
                                this.strategy.on_step(&step, &this.history, &mut this.state);
                            let date = this.view.step_ord_key(&step);
                            let order = this.engine.transform(
                                output,
                                &step,
                                date,
                                &mut this.engine_state,
                            );
                            this.view.append(&mut this.history, &step);
                            match order {
                                Ok(Some(order)) => {
                                    return Poll::Ready(Some(Ok(OutputRow { output: order })))
                                }
                                Ok(None) => continue,
                                Err(err) => return Poll::Ready(Some(Err(err))),
                            }
                        }
                        Poll::Ready(None) => return Poll::Ready(None),
                        Poll::Pending => return Poll::Pending,
                    }
                }
            }
        }

        RunStream {
            engine: &self.engine,
            strategy: &self.strategy,
            view,
            stream,
            history: DataFrame::empty(),
            state: S::State::default(),
            engine_state: E::State::default(),
        }
    }
}

impl<E, S: StrategyBase> AsyncExecutor<E, S> {
    /// Multi-key variant of [`AsyncExecutor::run`].
    pub fn run_multi<V, St>(
        &mut self,
        streams: HashMap<S::Key, St>,
        views: HashMap<S::Key, V>,
    ) -> impl Stream<Item = Result<OutputRow<E::Order>, OrderError>> + '_
    where
        V: View<Item = S::Input> + 'static,
        St: Stream<Item = S::Input> + Unpin + 'static,
        S::Input: 'static,
        S::Key: 'static,
        E: OrderEngine<S::Output, S::Input>,
    {
        use std::pin::Pin;
        use std::task::{Context, Poll};

        struct RunStream<'a, E2, S2, V2, St2>
        where
            S2: StrategyBase,
            V2: View<Item = S2::Input>,
            St2: Stream<Item = S2::Input> + Unpin,
            E2: OrderEngine<S2::Output, S2::Input>,
        {
            engine: &'a E2,
            strategy: &'a S2,
            views: HashMap<S2::Key, V2>,
            streams: Vec<(S2::Key, St2)>,
            history: HashMap<S2::Key, DataFrame>,
            state: S2::State,
            engine_state: E2::State,
        }

        impl<'a, E2, S2, V2, St2> Stream for RunStream<'a, E2, S2, V2, St2>
        where
            S2: StrategyBase,
            V2: View<Item = S2::Input>,
            St2: Stream<Item = S2::Input> + Unpin,
            E2: OrderEngine<S2::Output, S2::Input>,
        {
            type Item = Result<OutputRow<E2::Order>, OrderError>;

            fn poll_next(
                self: Pin<&mut Self>,
                cx: &mut Context<'_>,
            ) -> Poll<Option<Self::Item>> {
                let this = unsafe { self.get_unchecked_mut() };

                'scan: loop {
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
                                let date = this.views[key].step_ord_key(&step);
                                let order = this.engine.transform(
                                    output,
                                    &step,
                                    date,
                                    &mut this.engine_state,
                                );
                                let hist_df = this
                                    .history
                                    .entry(key.clone())
                                    .or_insert_with(DataFrame::empty);
                                this.views[key].append(hist_df, &step);

                                match order {
                                    Ok(Some(order)) => {
                                        return Poll::Ready(Some(Ok(OutputRow { output: order })))
                                    }
                                    Err(err) => return Poll::Ready(Some(Err(err))),
                                    Ok(None) => continue 'scan,
                                }
                            }
                            Poll::Ready(None) => {}
                            Poll::Pending => {
                                all_exhausted = false;
                            }
                        }
                    }

                    return if all_exhausted {
                        Poll::Ready(None)
                    } else {
                        Poll::Pending
                    };
                }
            }
        }

        let mut history = HashMap::new();
        for key in views.keys() {
            history.insert(key.clone(), DataFrame::empty());
        }

        RunStream {
            engine: &self.engine,
            strategy: &self.strategy,
            views,
            streams: streams.into_iter().collect(),
            history,
            state: S::State::default(),
            engine_state: E::State::default(),
        }
    }
}
