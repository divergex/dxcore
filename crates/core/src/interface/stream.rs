use futures::{Future, Stream};
use std::pin::Pin;
use std::time::Duration;
use tokio_stream::{wrappers::IntervalStream, StreamExt};

/// Polls `f` at `period` intervals, yielding each result as a [`Stream`] item.
///
/// When `immediate` is true, the first call fires on subscription instead of
/// after the first `period` elapses.
///
/// # Example
///
/// ```ignore
/// let prices = poll(Duration::from_secs(5), false, || async {
///     reqwest::get("https://api.example.com/price")
///         .await?
///         .json::<PriceData>()
///         .await
///         .map_err(|e| Error::Http(e.to_string()))
/// });
/// ```
pub fn poll<F, Fut, T, E>(
    period: Duration,
    immediate: bool,
    f: F,
) -> Pin<Box<dyn Stream<Item = Result<T, E>> + Send>>
where
    F: Fn() -> Fut + Send + 'static,
    Fut: Future<Output = Result<T, E>> + Send + 'static,
    T: Send + 'static,
    E: Send + 'static,
{
    let interval = if immediate {
        tokio::time::interval_at(tokio::time::Instant::now(), period)
    } else {
        tokio::time::interval(period)
    };
    Box::pin(IntervalStream::new(interval).then(move |_| f()))
}

/// Like [`poll`], but also invokes `callback` with a reference to each result
/// before yielding it to the stream consumer.
pub fn poll_callback<F, Fut, C, T, E>(
    period: Duration,
    immediate: bool,
    f: F,
    callback: C,
) -> Pin<Box<dyn Stream<Item = Result<T, E>> + Send>>
where
    F: Fn() -> Fut + Send + 'static,
    Fut: Future<Output = Result<T, E>> + Send + 'static,
    C: Fn(&Result<T, E>) + Send + Sync + 'static,
    T: Send + 'static,
    E: Send + 'static,
{
    let interval = if immediate {
        tokio::time::interval_at(tokio::time::Instant::now(), period)
    } else {
        tokio::time::interval(period)
    };
    let cb = std::sync::Arc::new(callback);
    Box::pin(IntervalStream::new(interval).then(move |_| {
        let fut = f();
        let cb = std::sync::Arc::clone(&cb);
        async move {
            let result = fut.await;
            cb(&result);
            result
        }
    }))
}
