mod sync;
mod async_;

pub use async_::AsyncExecutor;
pub use sync::SyncExecutor;

pub struct OutputRow<O> {
    pub output: O,
}

pub(crate) struct TaggedStep<K, I> {
    pub key: K,
    pub step: I,
    pub ord: Option<i64>,
}
