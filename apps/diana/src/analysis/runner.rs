use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread;

use super::query::QueryError;

/// Each worker can queue one completed row; channel closure propagates sink
/// failure directly to producers.
enum Message<T> {
    Row(T),
    Failed(QueryError),
}

pub(super) fn run<T, P>(
    images: usize,
    jobs: usize,
    produce: P,
    sink: &mut impl FnMut(T) -> Result<(), QueryError>,
) -> Result<(), QueryError>
where
    T: Send,
    P: Fn(usize, &mut dyn FnMut(T) -> Result<(), QueryError>) -> Result<(), QueryError> + Sync,
{
    let workers = jobs.min(images.max(1));
    let (sender, receiver) = sync_channel::<Message<T>>(workers);
    thread::scope(|scope| {
        for worker in 0..workers {
            let sender = sender.clone();
            let produce = &produce;
            scope.spawn(move || worker_loop(worker, workers, images, produce, &sender));
        }
        drop(sender);
        consume(receiver, sink)
    })
}

fn worker_loop<T, P>(
    worker: usize,
    workers: usize,
    images: usize,
    produce: &P,
    sender: &SyncSender<Message<T>>,
) where
    T: Send,
    P: Fn(usize, &mut dyn FnMut(T) -> Result<(), QueryError>) -> Result<(), QueryError> + Sync,
{
    for ordinal in (worker..images).step_by(workers) {
        let mut emit = |row| {
            sender
                .send(Message::Row(row))
                .map_err(|_| QueryError::WorkerLost)
        };
        if let Err(error) = produce(ordinal, &mut emit) {
            let _ = sender.send(Message::Failed(error));
            return;
        }
    }
}

fn consume<T>(
    receiver: Receiver<Message<T>>,
    sink: &mut impl FnMut(T) -> Result<(), QueryError>,
) -> Result<(), QueryError> {
    while let Ok(message) = receiver.recv() {
        match message {
            Message::Row(row) => sink(row)?,
            Message::Failed(error) => return Err(error),
        }
    }
    Ok(())
}
