use polars::prelude::*;

use super::{Strategy, View};

pub struct SyncExecutor<S: Strategy> {
    pub strategy: S,
}

impl<S: Strategy> SyncExecutor<S> {
    pub fn new(strategy: S) -> Self {
        Self { strategy }
    }

    pub fn run<V: View<Item = S::Input>>(&mut self, df: &DataFrame, view: V) -> S::Frame {
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

pub struct OutputRow<O> {
    pub output: O,
}

/// Streaming executor: processes items from a [`futures::Stream`]
pub struct AsyncExecutor<S: Strategy> {
    pub strategy: S,
}

impl<S: Strategy> AsyncExecutor<S> {
    pub fn new(strategy: S) -> Self {
        Self { strategy }
    }

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

        struct RunStream<'a, S2: Strategy, V2: View<Item = S2::Input>, St2: Stream<Item = S2::Input> + Unpin> {
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

            fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
                let this = unsafe { self.get_unchecked_mut() };
                match Pin::new(&mut this.stream).poll_next(cx) {
                    Poll::Ready(Some(step)) => {
                        let output = this.strategy.on_step(&step, &this.history, &mut this.state);
                        this.strategy.append_output(&mut this.frame, output.clone(), &step);
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
