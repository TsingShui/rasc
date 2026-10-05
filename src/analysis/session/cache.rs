//! A byte-budgeted cache exclusively for ZIP deflate output.
use super::{Result, SessionError};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(crate) struct InflateKey {
    pub target: String,
    pub entry: usize,
}

struct Cached {
    bytes: Arc<Vec<u8>>,
    touched: u64,
}

struct State {
    entries: HashMap<InflateKey, Cached>,
    loading: HashSet<InflateKey>,
    used: usize,
    reserved: usize,
    clock: u64,
    loads: u64,
    hits: u64,
}

pub(crate) struct InflateCache {
    max_bytes: usize,
    state: Mutex<State>,
    changed: Condvar,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CacheStats {
    pub bytes: usize,
    pub entries: usize,
    pub loads: u64,
    pub hits: u64,
}

impl InflateCache {
    pub(crate) fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            state: Mutex::new(State {
                entries: HashMap::new(),
                loading: HashSet::new(),
                used: 0,
                reserved: 0,
                clock: 0,
                loads: 0,
                hits: 0,
            }),
            changed: Condvar::new(),
        }
    }

    pub(crate) fn get_or_load(
        &self,
        key: InflateKey,
        expected: usize,
        target_cancel: &CancellationToken,
        request_cancel: Option<&CancellationToken>,
        load: impl FnOnce() -> Result<Vec<u8>>,
    ) -> Result<Arc<Vec<u8>>> {
        self.get_or_load_with_publish_hook(
            key,
            expected,
            target_cancel,
            request_cancel,
            load,
            || {},
        )
    }

    fn get_or_load_with_publish_hook(
        &self,
        key: InflateKey,
        expected: usize,
        target_cancel: &CancellationToken,
        request_cancel: Option<&CancellationToken>,
        load: impl FnOnce() -> Result<Vec<u8>>,
        before_publish_lock: impl FnOnce(),
    ) -> Result<Arc<Vec<u8>>> {
        if expected > self.max_bytes {
            return Err(SessionError::limit(format!(
                "entry needs {expected} bytes, over the {} byte inflate cache limit",
                self.max_bytes
            )));
        }
        let mut state = self.state.lock().expect("byte cache lock poisoned");
        loop {
            state.clock += 1;
            let touched = state.clock;
            if let Some(hit) = state.entries.get_mut(&key) {
                check_cancel(target_cancel, request_cancel)?;
                hit.touched = touched;
                let bytes = Arc::clone(&hit.bytes);
                state.hits += 1;
                return Ok(bytes);
            }
            if state.loading.contains(&key) {
                check_cancel(target_cancel, request_cancel)?;
                let (next, _) = self
                    .changed
                    .wait_timeout(state, std::time::Duration::from_millis(50))
                    .expect("byte cache lock poisoned");
                state = next;
                continue;
            }
            evict_until(&mut state, self.max_bytes.saturating_sub(expected));
            if state
                .used
                .saturating_add(state.reserved)
                .saturating_add(expected)
                > self.max_bytes
            {
                return Err(SessionError::limit(
                    "inflate cache is full with data still in use; close a target or retry later",
                ));
            }
            state.loading.insert(key.clone());
            state.reserved += expected;
            break;
        }
        drop(state);

        let loaded = if target_cancel.is_cancelled() {
            Err(SessionError::new("CANCELLED", "target was closed"))
        } else if request_cancel.is_some_and(CancellationToken::is_cancelled) {
            Err(SessionError::new("CANCELLED", "analysis was cancelled"))
        } else {
            load()
        };
        // Inflation itself is not interruptible. Request cancellation after a
        // successful load does not invalidate bytes that another request can
        // reuse. Target cancellation is checked again while holding the cache
        // lock immediately before publication, which closes the remove/publish
        // race with `remove_target`.
        before_publish_lock();
        let mut state = self.state.lock().expect("byte cache lock poisoned");
        state.loading.remove(&key);
        state.reserved = state.reserved.saturating_sub(expected);
        let result = match loaded {
            Ok(_) if target_cancel.is_cancelled() => {
                Err(SessionError::new("CANCELLED", "target was closed"))
            }
            Ok(bytes) => {
                let actual = bytes.len();
                evict_until(&mut state, self.max_bytes.saturating_sub(actual));
                if state
                    .used
                    .saturating_add(state.reserved)
                    .saturating_add(actual)
                    > self.max_bytes
                {
                    Err(SessionError::limit(
                        "inflated entry does not fit the configured cache budget",
                    ))
                } else {
                    state.clock += 1;
                    let bytes = Arc::new(bytes);
                    let touched = state.clock;
                    state.used += actual;
                    state.loads += 1;
                    state.entries.insert(
                        key,
                        Cached {
                            bytes: Arc::clone(&bytes),
                            touched,
                        },
                    );
                    Ok(bytes)
                }
            }
            Err(error) => Err(error),
        };
        self.changed.notify_all();
        result
    }

    pub(crate) fn remove_target(&self, target: &str) {
        let mut state = self.state.lock().expect("byte cache lock poisoned");
        let keys = state
            .entries
            .keys()
            .filter(|key| key.target == target)
            .cloned()
            .collect::<Vec<_>>();
        for key in keys {
            if let Some(entry) = state.entries.remove(&key) {
                state.used = state.used.saturating_sub(entry.bytes.len());
            }
        }
        self.changed.notify_all();
    }

    pub(crate) fn stats(&self) -> CacheStats {
        let state = self.state.lock().expect("byte cache lock poisoned");
        CacheStats {
            bytes: state.used,
            entries: state.entries.len(),
            loads: state.loads,
            hits: state.hits,
        }
    }
}

fn check_cancel(
    target_cancel: &CancellationToken,
    request_cancel: Option<&CancellationToken>,
) -> Result<()> {
    if target_cancel.is_cancelled() {
        return Err(SessionError::new("CANCELLED", "target was closed"));
    }
    if request_cancel.is_some_and(CancellationToken::is_cancelled) {
        return Err(SessionError::new("CANCELLED", "analysis was cancelled"));
    }
    Ok(())
}

fn evict_until(state: &mut State, maximum_used: usize) {
    while state.used.saturating_add(state.reserved) > maximum_used {
        let victim = state
            .entries
            .iter()
            .filter(|(_, cached)| Arc::strong_count(&cached.bytes) == 1)
            .min_by_key(|(_, cached)| cached.touched)
            .map(|(key, _)| key.clone());
        let Some(victim) = victim else {
            return;
        };
        if let Some(removed) = state.entries.remove(&victim) {
            state.used = state.used.saturating_sub(removed.bytes.len());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn key(entry: usize) -> InflateKey {
        InflateKey {
            target: "target".into(),
            entry,
        }
    }

    #[test]
    fn hits_and_lru_eviction_stay_inside_budget() {
        let cache = InflateCache::new(8);
        let cancel = CancellationToken::new();
        let first = cache
            .get_or_load(key(1), 4, &cancel, None, || Ok(vec![1; 4]))
            .unwrap();
        assert_eq!(
            &*cache
                .get_or_load(key(1), 4, &cancel, None, || unreachable!())
                .unwrap(),
            &[1; 4]
        );
        drop(first);
        cache
            .get_or_load(key(2), 6, &cancel, None, || Ok(vec![2; 6]))
            .unwrap();
        let stats = cache.stats();
        assert_eq!(stats.bytes, 6);
        assert_eq!(stats.entries, 1);
        assert_eq!(stats.loads, 2);
        assert_eq!(stats.hits, 1);
    }

    #[test]
    fn failed_and_cancelled_loads_release_reservations() {
        let cache = InflateCache::new(8);
        let cancel = CancellationToken::new();
        let error = cache
            .get_or_load(key(1), 8, &cancel, None, || {
                Err(SessionError::invalid("broken"))
            })
            .unwrap_err();
        assert_eq!(error.code, "INVALID_INPUT");
        assert_eq!(cache.stats().bytes, 0);
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        assert_eq!(
            cache
                .get_or_load(key(1), 8, &cancelled, None, || unreachable!())
                .unwrap_err()
                .code,
            "CANCELLED"
        );
        assert_eq!(cache.stats().bytes, 0);
        assert_eq!(
            &*cache
                .get_or_load(key(1), 8, &cancel, None, || Ok(vec![9; 8]))
                .unwrap(),
            &[9; 8]
        );
        assert_eq!(cache.stats().loads, 1);
    }

    #[test]
    fn close_during_load_does_not_publish_the_finished_bytes() {
        let cache = Arc::new(InflateCache::new(16));
        let cancel = CancellationToken::new();
        let started = Arc::new(std::sync::Barrier::new(2));
        let finish = Arc::new(std::sync::Barrier::new(2));
        std::thread::scope(|scope| {
            let worker_cache = Arc::clone(&cache);
            let worker_cancel = cancel.clone();
            let started_worker = Arc::clone(&started);
            let finish_worker = Arc::clone(&finish);
            let worker = scope.spawn(move || {
                worker_cache.get_or_load(key(1), 8, &worker_cancel, None, || {
                    started_worker.wait();
                    finish_worker.wait();
                    Ok(vec![1; 8])
                })
            });
            started.wait();
            cancel.cancel();
            cache.remove_target("target");
            finish.wait();
            assert_eq!(worker.join().unwrap().unwrap_err().code, "CANCELLED");
        });
        let stats = cache.stats();
        assert_eq!(stats.entries, 0);
        assert_eq!(stats.bytes, 0);
        assert_eq!(stats.loads, 0);
    }

    #[test]
    fn close_between_inflate_and_publication_does_not_publish() {
        let cache = Arc::new(InflateCache::new(16));
        let cancel = CancellationToken::new();
        let before_publish = Arc::new(std::sync::Barrier::new(2));
        let continue_publish = Arc::new(std::sync::Barrier::new(2));
        std::thread::scope(|scope| {
            let worker_cache = Arc::clone(&cache);
            let worker_cancel = cancel.clone();
            let before_worker = Arc::clone(&before_publish);
            let continue_worker = Arc::clone(&continue_publish);
            let worker = scope.spawn(move || {
                worker_cache.get_or_load_with_publish_hook(
                    key(1),
                    8,
                    &worker_cancel,
                    None,
                    || Ok(vec![1; 8]),
                    || {
                        before_worker.wait();
                        continue_worker.wait();
                    },
                )
            });
            before_publish.wait();
            cancel.cancel();
            cache.remove_target("target");
            continue_publish.wait();
            assert_eq!(worker.join().unwrap().unwrap_err().code, "CANCELLED");
        });
        let stats = cache.stats();
        assert_eq!(stats.entries, 0);
        assert_eq!(stats.bytes, 0);
        assert_eq!(stats.loads, 0);
    }

    #[test]
    fn request_cancel_stops_a_single_flight_waiter_without_stopping_the_loader() {
        let cache = Arc::new(InflateCache::new(32));
        let target_cancel = CancellationToken::new();
        let request_cancel = CancellationToken::new();
        let started = Arc::new(std::sync::Barrier::new(2));
        let finish = Arc::new(std::sync::Barrier::new(2));
        std::thread::scope(|scope| {
            let loader_cache = Arc::clone(&cache);
            let loader_target = target_cancel.clone();
            let loader_started = Arc::clone(&started);
            let loader_finish = Arc::clone(&finish);
            let loader = scope.spawn(move || {
                loader_cache.get_or_load(key(1), 8, &loader_target, None, || {
                    loader_started.wait();
                    loader_finish.wait();
                    Ok(vec![4; 8])
                })
            });
            started.wait();
            let waiter_cache = Arc::clone(&cache);
            let waiter_target = target_cancel.clone();
            let waiter_cancel = request_cancel.clone();
            let waiter = scope.spawn(move || {
                waiter_cache.get_or_load(
                    key(1),
                    8,
                    &waiter_target,
                    Some(&waiter_cancel),
                    || unreachable!(),
                )
            });
            request_cancel.cancel();
            let error = waiter.join().unwrap().unwrap_err();
            assert_eq!(error.code, "CANCELLED");
            finish.wait();
            assert_eq!(&*loader.join().unwrap().unwrap(), &[4; 8]);
        });
        assert_eq!(cache.stats().loads, 1);
    }

    #[test]
    fn concurrent_miss_is_loaded_once() {
        let cache = Arc::new(InflateCache::new(32));
        let loads = Arc::new(AtomicUsize::new(0));
        std::thread::scope(|scope| {
            for _ in 0..8 {
                let cache = Arc::clone(&cache);
                let loads = Arc::clone(&loads);
                scope.spawn(move || {
                    let cancel = CancellationToken::new();
                    cache
                        .get_or_load(key(1), 8, &cancel, None, || {
                            loads.fetch_add(1, Ordering::Relaxed);
                            std::thread::sleep(std::time::Duration::from_millis(10));
                            Ok(vec![7; 8])
                        })
                        .unwrap();
                });
            }
        });
        assert_eq!(loads.load(Ordering::Relaxed), 1);
    }
}
