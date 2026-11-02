use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread;

use super::query::{QueryError, QueryLimit, QueryLimits};

/// One worker message. A batch is bounded by the configured batch size, and the
/// channel capacity equals the worker count, so pending memory stays bounded.
enum Message<T> {
    Batch(Vec<T>),
    Failed(QueryError),
}

/// Streams rows from independent root images in completion order.
///
/// `produce` is called once per admitted image; it pushes rows through the given
/// emitter and must respect `cancel`. Rows are delivered as workers finish: there
/// is no ordinal wait, global sort or reorder buffer. A configured row bound, a
/// cancellation or a producer/sink failure stops the request and never turns the
/// delivered prefix into a success.
pub(super) fn run<T, P>(
    images: usize,
    jobs: usize,
    cancel: &AtomicBool,
    limits: QueryLimits,
    produce: P,
    sink: &mut impl FnMut(T) -> Result<(), QueryError>,
) -> Result<(), QueryError>
where
    T: Send,
    P: Fn(
            usize,
            &AtomicBool,
            &mut dyn FnMut(T) -> Result<(), QueryError>,
        ) -> Result<(), QueryError>
        + Sync,
{
    let workers = jobs.min(images.max(1));
    let (sender, receiver) = sync_channel::<Message<T>>(workers);
    let mut failure = None;
    thread::scope(|scope| {
        for worker in 0..workers {
            let sender = sender.clone();
            let produce = &produce;
            scope.spawn(move || {
                worker_loop(worker, workers, images, produce, &sender, cancel, limits);
            });
        }
        drop(sender);
        consume(receiver, cancel, limits, sink, &mut failure);
        if failure.is_some() {
            // Stop the workers of a failed request; a completed pass leaves the
            // caller's flag untouched so the scope can be reused.
            cancel.store(true, Ordering::Relaxed);
        }
    });
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn worker_loop<T, P>(
    worker: usize,
    workers: usize,
    images: usize,
    produce: &P,
    sender: &SyncSender<Message<T>>,
    cancel: &AtomicBool,
    limits: QueryLimits,
) where
    T: Send,
    P: Fn(
            usize,
            &AtomicBool,
            &mut dyn FnMut(T) -> Result<(), QueryError>,
        ) -> Result<(), QueryError>
        + Sync,
{
    let mut batch = Batch {
        rows: Vec::new(),
        batch_rows: limits.max_batch_rows.unwrap_or(1).max(1),
        sender,
        cancel,
    };
    for ordinal in (worker..images).step_by(workers) {
        let result = {
            let mut emit = |row: T| batch.push(row);
            produce(ordinal, cancel, &mut emit)
        };
        let result = match result {
            Ok(()) => batch.flush(),
            Err(error) => Err(error),
        };
        if let Err(error) = result {
            let _ = sender.send(Message::Failed(error));
            return;
        }
    }
}

struct Batch<'a, T> {
    rows: Vec<T>,
    batch_rows: u64,
    sender: &'a SyncSender<Message<T>>,
    cancel: &'a AtomicBool,
}

impl<T> Batch<'_, T> {
    fn push(&mut self, row: T) -> Result<(), QueryError> {
        self.rows.push(row);
        if u64::try_from(self.rows.len()).unwrap_or(u64::MAX) >= self.batch_rows {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), QueryError> {
        if self.rows.is_empty() {
            return Ok(());
        }
        if self.cancel.load(Ordering::Relaxed) {
            return Err(QueryError::Cancelled);
        }
        let rows = std::mem::take(&mut self.rows);
        self.sender
            .send(Message::Batch(rows))
            .map_err(|_| QueryError::WorkerLost)
    }
}

fn consume<T>(
    receiver: Receiver<Message<T>>,
    cancel: &AtomicBool,
    limits: QueryLimits,
    sink: &mut impl FnMut(T) -> Result<(), QueryError>,
    failure: &mut Option<QueryError>,
) {
    let mut delivered = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            *failure = Some(QueryError::Cancelled);
            break;
        }
        match receiver.recv() {
            Ok(Message::Batch(rows)) => {
                let count = u64::try_from(rows.len()).unwrap_or(u64::MAX);
                delivered = match delivered.checked_add(count) {
                    Some(value) => value,
                    None => {
                        *failure = Some(QueryError::WorkerLost);
                        break;
                    }
                };
                if let Some(maximum) = limits.max_rows
                    && delivered > maximum
                {
                    *failure = Some(QueryError::Budget {
                        limit: QueryLimit::Rows,
                        requested: delivered,
                        maximum,
                    });
                    break;
                }
                for row in rows {
                    if let Err(error) = sink(row) {
                        *failure = Some(error);
                        return;
                    }
                }
            }
            Ok(Message::Failed(error)) => {
                *failure = Some(error);
                break;
            }
            Err(_) => break,
        }
    }
}
