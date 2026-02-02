use tokio::select;
use tokio_util::sync::CancellationToken;

#[derive(PartialEq, Eq)]
pub enum CancelResult<T> {
    Cancelled,
    Success(T),
}

pub async fn cancel_select<T>(
    cancel: &CancellationToken,
    f: impl Future<Output = T>,
) -> CancelResult<T> {
    select! {
        res = f => CancelResult::Success(res),
        _ = cancel.cancelled() => CancelResult::Cancelled
    }
}
