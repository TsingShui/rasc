use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread;

use super::query::QueryError;

/// Each worker can queue one completed row; the bounded channel applies
/// backpressure independently of the row's byte length.
enum Message<T> {
    Row(T),
    Failed(QueryError),
}

/// Streams independent root images in completion order. Cancellation and
/// producer/sink failure stop the request; a delivered prefix is not success.
pub(super) fn run<T, P>(
    images: usize,
    jobs: usize,
    cancel: &AtomicBool,
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
            scope.spawn(move || worker_loop(worker, workers, images, produce, &sender, cancel));
        }
        drop(sender);
        consume(receiver, cancel, sink, &mut failure);
        if failure.is_some() {
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
) where
    T: Send,
    P: Fn(
            usize,
            &AtomicBool,
            &mut dyn FnMut(T) -> Result<(), QueryError>,
        ) -> Result<(), QueryError>
        + Sync,
{
    for ordinal in (worker..images).step_by(workers) {
        let mut emit = |row| {
            if cancel.load(Ordering::Relaxed) {
                return Err(QueryError::Cancelled);
            }
            sender
                .send(Message::Row(row))
                .map_err(|_| QueryError::WorkerLost)
        };
        if let Err(error) = produce(ordinal, cancel, &mut emit) {
            let _ = sender.send(Message::Failed(error));
            return;
        }
    }
}

fn consume<T>(
    receiver: Receiver<Message<T>>,
    cancel: &AtomicBool,
    sink: &mut impl FnMut(T) -> Result<(), QueryError>,
    failure: &mut Option<QueryError>,
) {
    loop {
        if cancel.load(Ordering::Relaxed) {
            *failure = Some(QueryError::Cancelled);
            break;
        }
        match receiver.recv() {
            Ok(Message::Row(row)) => {
                if let Err(error) = sink(row) {
                    *failure = Some(error);
                    break;
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
