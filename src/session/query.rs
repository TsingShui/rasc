//! Typed, complete query results for protocol adapters.
use super::{AnalysisSession, Result, SessionError, Target, result_limit_error};
use crate::query::{ClassQuery, MemberQuery, Query};
use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, RecvTimeoutError, sync_channel},
};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct ClassItem {
    pub class_id: String,
    pub dex_id: String,
    pub descriptor: String,
    pub dotted_name: String,
    pub entry_index: usize,
    pub logical_index: usize,
    pub class_def_idx: usize,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct ClassList {
    pub items: Vec<ClassItem>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct StringItem {
    pub dex_id: String,
    pub string_index: usize,
    pub value: String,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct StringList {
    pub items: Vec<StringItem>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FindRefsKind {
    String,
    Type,
    Method,
    Field,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct ReferenceItem {
    pub dex_id: String,
    pub method_index: u32,
    pub referencing_member: String,
    pub matched: String,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct ReferenceList {
    pub items: Vec<ReferenceItem>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct SourceResult {
    pub class_id: String,
    pub dex_id: String,
    pub source: String,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct ManifestResult {
    pub xml: String,
}

struct Logical<'a> {
    dex_id: String,
    entry_index: usize,
    logical_index: usize,
    data: Cow<'a, [u8]>,
}

enum EntryEvent<T> {
    Items(Vec<(T, usize)>, usize),
    Error(SessionError),
    Done,
}

const ROW_BATCH: usize = 64;
const ROW_BUFFER: usize = 32;

struct RowGate {
    buffered: Mutex<RowGateState>,
    changed: Condvar,
    max_bytes: usize,
    max_rows: usize,
}

struct RowGateState {
    bytes: usize,
    rows: usize,
    per_entry_bytes: HashMap<usize, usize>,
    per_entry_rows: HashMap<usize, usize>,
}

impl RowGate {
    fn new(max_bytes: usize, max_rows: usize) -> Self {
        Self {
            buffered: Mutex::new(RowGateState {
                bytes: 0,
                rows: 0,
                per_entry_bytes: HashMap::new(),
                per_entry_rows: HashMap::new(),
            }),
            changed: Condvar::new(),
            max_bytes,
            max_rows,
        }
    }

    fn acquire(&self, ordinal: usize, bytes: usize, rows: usize, stop: &AtomicBool) -> bool {
        let mut buffered = self.buffered.lock().expect("row gate lock poisoned");
        while (buffered.per_entry_bytes.get(&ordinal).copied().unwrap_or(0) != 0
            && buffered.bytes.saturating_add(bytes) > self.max_bytes
            || buffered.per_entry_rows.get(&ordinal).copied().unwrap_or(0) != 0
                && buffered.rows.saturating_add(rows) > self.max_rows)
            && !stop.load(Ordering::Acquire)
        {
            let (next, _) = self
                .changed
                .wait_timeout(buffered, Duration::from_millis(10))
                .expect("row gate lock poisoned");
            buffered = next;
        }
        if stop.load(Ordering::Acquire) {
            return false;
        }
        buffered.bytes = buffered.bytes.saturating_add(bytes);
        buffered.rows = buffered.rows.saturating_add(rows);
        let entry_bytes = buffered.per_entry_bytes.entry(ordinal).or_default();
        *entry_bytes = entry_bytes.saturating_add(bytes);
        let entry_rows = buffered.per_entry_rows.entry(ordinal).or_default();
        *entry_rows = entry_rows.saturating_add(rows);
        true
    }

    fn release(&self, ordinal: usize, bytes: usize, rows: usize) {
        let mut buffered = self.buffered.lock().expect("row gate lock poisoned");
        buffered.bytes = buffered.bytes.saturating_sub(bytes);
        buffered.rows = buffered.rows.saturating_sub(rows);
        if let Some(entry_bytes) = buffered.per_entry_bytes.get_mut(&ordinal) {
            *entry_bytes = entry_bytes.saturating_sub(bytes);
            if *entry_bytes == 0 {
                buffered.per_entry_bytes.remove(&ordinal);
            }
        }
        if let Some(entry_rows) = buffered.per_entry_rows.get_mut(&ordinal) {
            *entry_rows = entry_rows.saturating_sub(rows);
            if *entry_rows == 0 {
                buffered.per_entry_rows.remove(&ordinal);
            }
        }
        self.changed.notify_all();
    }
}

struct EntryGate {
    state: Mutex<EntryGateState>,
    changed: Condvar,
    max_weight: usize,
    max_entries_ahead: usize,
}

struct EntryGateState {
    next_ordinal: usize,
    consumed: usize,
    active_weight: usize,
    active_entries: usize,
}

impl EntryGate {
    fn new(max_weight: usize, max_entries_ahead: usize) -> Self {
        Self {
            state: Mutex::new(EntryGateState {
                next_ordinal: 0,
                consumed: 0,
                active_weight: 0,
                active_entries: 0,
            }),
            changed: Condvar::new(),
            max_weight,
            max_entries_ahead,
        }
    }

    fn acquire(
        &self,
        ordinal: usize,
        weight: usize,
        stop: &AtomicBool,
        target: &Target,
        request_cancel: Option<&CancellationToken>,
    ) -> bool {
        let mut state = self.state.lock().expect("entry gate lock poisoned");
        loop {
            if stop.load(Ordering::Acquire)
                || target.cancel.is_cancelled()
                || request_cancel.is_some_and(CancellationToken::is_cancelled)
            {
                stop.store(true, Ordering::Release);
                self.changed.notify_all();
                return false;
            }
            let fits = state.active_entries == 0
                || state.active_weight.saturating_add(weight) <= self.max_weight;
            let inside_window = ordinal < state.consumed.saturating_add(self.max_entries_ahead);
            if ordinal == state.next_ordinal && inside_window && fits {
                state.next_ordinal += 1;
                state.active_entries += 1;
                state.active_weight = state.active_weight.saturating_add(weight);
                self.changed.notify_all();
                return true;
            }
            let (next, _) = self
                .changed
                .wait_timeout(state, Duration::from_millis(10))
                .expect("entry gate lock poisoned");
            state = next;
        }
    }

    fn release(&self, weight: usize) {
        let mut state = self.state.lock().expect("entry gate lock poisoned");
        state.active_entries = state.active_entries.saturating_sub(1);
        state.active_weight = state.active_weight.saturating_sub(weight);
        self.changed.notify_all();
    }

    fn mark_consumed(&self, next_ordinal: usize) {
        let mut state = self.state.lock().expect("entry gate lock poisoned");
        state.consumed = next_ordinal;
        self.changed.notify_all();
    }

    fn stop(&self) {
        self.changed.notify_all();
    }
}

impl AnalysisSession {
    pub fn classes(&self, target_id: &str) -> Result<ClassList> {
        self.classes_cancellable(target_id, None)
    }

    pub(crate) fn classes_cancellable(
        &self,
        target_id: &str,
        request_cancel: Option<&CancellationToken>,
    ) -> Result<ClassList> {
        let target = self.target(target_id)?;
        let mut budget = self.result_budget();
        let items = self.parallel_entry_items(
            &target,
            request_cancel,
            4 << 20,
            |logical, send, stop| {
                let mut stopped = None;
                crate::dex::for_each_class_definition(&logical.data, |definition| {
                    if stop.load(Ordering::Acquire) {
                        return Ok(false);
                    }
                    if let Err(error) = check_cancel(&target, request_cancel) {
                        stopped = Some(error);
                        return Ok(false);
                    }
                    let dotted_name = definition
                        .descriptor
                        .strip_prefix('L')
                        .and_then(|name| name.strip_suffix(';'))
                        .unwrap_or(&definition.descriptor)
                        .replace('/', ".");
                    let item = ClassItem {
                        class_id: format!(
                            "{}:{}:{}",
                            logical.entry_index, logical.logical_index, definition.class_def_idx
                        ),
                        dex_id: logical.dex_id.clone(),
                        descriptor: definition.descriptor,
                        dotted_name,
                        entry_index: logical.entry_index,
                        logical_index: logical.logical_index,
                        class_def_idx: definition.class_def_idx,
                    };
                    Ok(send(item))
                })
                .map_err(SessionError::invalid)?;
                stopped.map_or(Ok(()), Err)
            },
            |_item, serialized_len| budget.add_serialized_len(serialized_len),
        )?;
        Ok(ClassList { items })
    }

    pub fn strings(&self, target_id: &str) -> Result<StringList> {
        self.strings_cancellable(target_id, None)
    }

    pub(crate) fn strings_cancellable(
        &self,
        target_id: &str,
        request_cancel: Option<&CancellationToken>,
    ) -> Result<StringList> {
        let target = self.target(target_id)?;
        let mut budget = self.result_budget();
        let items = self.parallel_entry_items(
            &target,
            request_cancel,
            32 << 20,
            |logical, send, stop| {
                let mut stopped = None;
                crate::dex::for_each_string(&logical.data, |string_index, raw| {
                    if stop.load(Ordering::Acquire) {
                        return Ok(false);
                    }
                    if let Err(error) = check_cancel(&target, request_cancel) {
                        stopped = Some(error);
                        return Ok(false);
                    }
                    Ok(send(StringItem {
                        dex_id: logical.dex_id.clone(),
                        string_index,
                        value: crate::dex::decode_string(raw),
                    }))
                })
                .map_err(SessionError::invalid)?;
                stopped.map_or(Ok(()), Err)
            },
            |_item, serialized_len| budget.add_serialized_len(serialized_len),
        )?;
        Ok(StringList { items })
    }

    pub fn findrefs(
        &self,
        target_id: &str,
        kind: FindRefsKind,
        value: Option<&str>,
        class: Option<&str>,
        fuzzy_class: bool,
    ) -> Result<ReferenceList> {
        self.findrefs_cancellable(target_id, kind, value, class, fuzzy_class, None)
    }

    pub(crate) fn findrefs_cancellable(
        &self,
        target_id: &str,
        kind: FindRefsKind,
        value: Option<&str>,
        class: Option<&str>,
        fuzzy_class: bool,
        request_cancel: Option<&CancellationToken>,
    ) -> Result<ReferenceList> {
        let target = self.target(target_id)?;
        let query = make_query(kind, value, class, fuzzy_class)?;
        let mut budget = self.result_budget();
        let items = self.parallel_entry_items(
            &target,
            request_cancel,
            4 << 20,
            |logical, send, stop| {
                let mut stopped = None;
                crate::dex::for_each_reference(
                    &logical.data,
                    &query,
                    self.config.max_result_bytes,
                    |row| {
                        if stop.load(Ordering::Acquire) {
                            return Ok(false);
                        }
                        if let Err(error) = check_cancel(&target, request_cancel) {
                            stopped = Some(error);
                            return Ok(false);
                        }
                        Ok(send(ReferenceItem {
                            dex_id: logical.dex_id.clone(),
                            method_index: row.method_index,
                            referencing_member: row.member,
                            matched: row.matched,
                        }))
                    },
                )
                .map_err(|error| {
                    if error
                        .downcast_ref::<crate::dex::ReferenceScratchLimit>()
                        .is_some()
                    {
                        SessionError::limit(error.to_string())
                    } else {
                        SessionError::invalid(error)
                    }
                })?;
                stopped.map_or(Ok(()), Err)
            },
            |_item, serialized_len| budget.add_serialized_len(serialized_len),
        )?;
        Ok(ReferenceList { items })
    }

    pub fn getclass(
        &self,
        target_id: &str,
        class_id: Option<&str>,
        class_name: Option<&str>,
    ) -> Result<SourceResult> {
        self.getclass_cancellable(target_id, class_id, class_name, None)
    }

    pub(crate) fn getclass_cancellable(
        &self,
        target_id: &str,
        class_id: Option<&str>,
        class_name: Option<&str>,
        request_cancel: Option<&CancellationToken>,
    ) -> Result<SourceResult> {
        let target = self.target(target_id)?;
        if class_id.is_some() == class_name.is_some() {
            return Err(SessionError::new(
                "INVALID_INPUT",
                "pass exactly one of class_id or class_name",
            ));
        }
        let descriptor = class_name
            .map(crate::query::format_class_name)
            .transpose()
            .map_err(SessionError::invalid)?;
        let mut candidates = Vec::new();
        self.for_each_logical(&target, request_cancel, |logical| {
            crate::dex::for_each_class_definition(&logical.data, |definition| {
                check_cancel(&target, request_cancel)?;
                let id = format!(
                    "{}:{}:{}",
                    logical.entry_index, logical.logical_index, definition.class_def_idx
                );
                if class_id.is_some_and(|wanted| wanted == id)
                    || descriptor
                        .as_ref()
                        .is_some_and(|wanted| wanted == &definition.descriptor)
                {
                    candidates.push(ClassItem {
                        class_id: id,
                        dex_id: logical.dex_id.clone(),
                        dotted_name: definition.descriptor.clone(),
                        descriptor: definition.descriptor,
                        entry_index: logical.entry_index,
                        logical_index: logical.logical_index,
                        class_def_idx: definition.class_def_idx,
                    });
                }
                Ok(candidates.len() < 2)
            })
            .map_err(SessionError::invalid)
        })?;
        match candidates.as_slice() {
            [] => return Err(SessionError::new("CLASS_NOT_FOUND", "class is not defined")),
            [_] => {}
            _ => {
                return Err(SessionError::new(
                    "AMBIGUOUS_CLASS",
                    "class has multiple definitions; pass class_id",
                ));
            }
        }
        let class = &candidates[0];
        let bytes = self.dex_bytes(&target, class.entry_index, request_cancel)?;
        let offsets =
            crate::dex::container::logical_offsets(&bytes).map_err(SessionError::invalid)?;
        let offset = *offsets.get(class.logical_index).ok_or_else(|| {
            SessionError::new("INVALID_INPUT", "class_id logical member is invalid")
        })?;
        let view =
            crate::dex::container::logical_view(&bytes, offset).map_err(SessionError::invalid)?;
        check_cancel(&target, request_cancel)?;
        let view = crate::dex::container::standard_header_cow(view);
        let source = self
            .analysis_pool
            .install(|| crate::emitter::render(&view, &class.descriptor))
            .map_err(SessionError::invalid)?;
        check_cancel(&target, request_cancel)?;
        let result = SourceResult {
            class_id: class.class_id.clone(),
            dex_id: class.dex_id.clone(),
            source,
        };
        self.ensure_result(&result)?;
        Ok(result)
    }

    fn parallel_entry_items<T: Send + Serialize>(
        &self,
        target: &Arc<Target>,
        request_cancel: Option<&CancellationToken>,
        minimum_parallel_entry_bytes: usize,
        produce: impl Fn(&Logical<'_>, &mut dyn FnMut(T) -> bool, &AtomicBool) -> Result<()> + Sync,
        mut commit: impl FnMut(&T, usize) -> Result<()>,
    ) -> Result<Vec<T>> {
        let count = target.dex_entries.len();
        let parallelizable_tail_bytes = parallelizable_entry_bytes(
            target
                .dex_entries
                .iter()
                .map(|&index| target.entries[index].uncompressed_size),
        );
        if count <= 1
            || self.config.analysis_threads <= 1
            || (!self.config.force_parallel_entries
                && parallelizable_tail_bytes < minimum_parallel_entry_bytes)
        {
            let mut items = Vec::new();
            let stop = AtomicBool::new(false);
            let mut stopped = None;
            self.for_each_logical(target, request_cancel, |logical| {
                let mut send = |item| {
                    let serialized_len = match serde_json::to_vec(&item) {
                        Ok(bytes) => bytes.len(),
                        Err(error) => {
                            stopped = Some(SessionError::invalid(error));
                            stop.store(true, Ordering::Release);
                            return false;
                        }
                    };
                    if let Err(error) = commit(&item, serialized_len) {
                        stopped = Some(error);
                        stop.store(true, Ordering::Release);
                        return false;
                    }
                    items.push(item);
                    true
                };
                produce(&logical, &mut send, &stop)?;
                stopped.take().map_or(Ok(()), Err)
            })?;
            return Ok(items);
        }

        let stop = AtomicBool::new(false);
        let gate = EntryGate::new(
            self.config.inflate_cache_bytes,
            self.config.analysis_threads,
        );
        let channel_capacity = ROW_BUFFER.saturating_mul(self.config.analysis_threads);
        let row_batch = ROW_BATCH.min(self.config.max_result_items.saturating_add(1).max(1));
        let (sender, receiver) = sync_channel(channel_capacity);
        let mut items = Vec::new();
        let row_gate = RowGate::new(
            self.config.max_result_bytes,
            channel_capacity.saturating_mul(row_batch),
        );
        let mut decisive_error = None;
        self.analysis_pool.in_place_scope_fifo(|scope| {
            for (ordinal, &entry_index) in target.dex_entries.iter().enumerate() {
                let sender = sender.clone();
                let weight = target.entries[entry_index]
                    .uncompressed_size
                    .min(self.config.inflate_cache_bytes);
                let stop = &stop;
                let gate = &gate;
                let target = Arc::clone(target);
                let produce = &produce;
                let row_gate = &row_gate;
                scope.spawn_fifo(move |_| {
                    if !gate.acquire(ordinal, weight, stop, &target, request_cancel) {
                        return;
                    }
                    let mut batch = Vec::with_capacity(row_batch);
                    let mut batch_bytes = 0usize;
                    let mut terminal_sent = false;
                    let result = self.for_each_logical_entry(
                        &target,
                        entry_index,
                        request_cancel,
                        |logical| {
                            let mut send = |item| {
                                let item_bytes = match serde_json::to_vec(&item) {
                                    Ok(bytes) => bytes.len(),
                                    Err(error) => {
                                        let _ = send_item_batch(
                                            &sender,
                                            ordinal,
                                            &mut batch,
                                            &mut batch_bytes,
                                            row_gate,
                                            row_batch,
                                            stop,
                                        );
                                        terminal_sent = send_event(
                                            &sender,
                                            (
                                                ordinal,
                                                EntryEvent::Error(SessionError::invalid(error)),
                                            ),
                                            stop,
                                        );
                                        return false;
                                    }
                                };
                                if item_bytes > self.config.max_result_bytes {
                                    let _ = send_item_batch(
                                        &sender,
                                        ordinal,
                                        &mut batch,
                                        &mut batch_bytes,
                                        row_gate,
                                        row_batch,
                                        stop,
                                    );
                                    terminal_sent = send_event(
                                        &sender,
                                        (
                                            ordinal,
                                            EntryEvent::Error(result_limit_error(
                                                self.config.max_result_items,
                                                self.config.max_result_bytes,
                                            )),
                                        ),
                                        stop,
                                    );
                                    return false;
                                }
                                if !batch.is_empty()
                                    && (batch.len() == row_batch
                                        || batch_bytes.saturating_add(item_bytes)
                                            > self.config.max_result_bytes)
                                    && !send_item_batch(
                                        &sender,
                                        ordinal,
                                        &mut batch,
                                        &mut batch_bytes,
                                        row_gate,
                                        row_batch,
                                        stop,
                                    )
                                {
                                    return false;
                                }
                                batch.push((item, item_bytes));
                                batch_bytes = batch_bytes.saturating_add(item_bytes);
                                true
                            };
                            produce(&logical, &mut send, stop)
                        },
                    );
                    if !terminal_sent
                        && send_item_batch(
                            &sender,
                            ordinal,
                            &mut batch,
                            &mut batch_bytes,
                            row_gate,
                            row_batch,
                            stop,
                        )
                    {
                        let event = match result {
                            Ok(()) => EntryEvent::Done,
                            Err(error) => EntryEvent::Error(error),
                        };
                        let _ = send_event(&sender, (ordinal, event), stop);
                    }
                    gate.release(weight);
                });
            }
            drop(sender);

            let mut ordinal = 0usize;
            let mut pending: HashMap<usize, VecDeque<EntryEvent<T>>> = HashMap::new();
            'entries: while ordinal < count {
                let event =
                    if let Some(event) = pending.get_mut(&ordinal).and_then(VecDeque::pop_front) {
                        event
                    } else {
                        match recv_event(&receiver, &stop, target, request_cancel) {
                            Ok((event_ordinal, event)) if event_ordinal == ordinal => event,
                            Ok((event_ordinal, event)) => {
                                pending.entry(event_ordinal).or_default().push_back(event);
                                continue;
                            }
                            Err(error) => {
                                decisive_error = Some(error);
                                stop.store(true, Ordering::Release);
                                gate.stop();
                                break 'entries;
                            }
                        }
                    };
                match event {
                    EntryEvent::Items(batch, buffered) => {
                        row_gate.release(ordinal, buffered, batch.len());
                        for (item, serialized_len) in batch {
                            if let Err(error) = commit(&item, serialized_len) {
                                decisive_error = Some(error);
                                stop.store(true, Ordering::Release);
                                gate.stop();
                                break 'entries;
                            }
                            items.push(item);
                        }
                    }
                    EntryEvent::Error(error) => {
                        decisive_error = Some(error);
                        stop.store(true, Ordering::Release);
                        gate.stop();
                        break 'entries;
                    }
                    EntryEvent::Done => {
                        ordinal += 1;
                        gate.mark_consumed(ordinal);
                    }
                }
            }
            stop.store(true, Ordering::Release);
            gate.stop();
            row_gate.changed.notify_all();
        });
        decisive_error.map_or(Ok(items), Err)
    }

    fn for_each_logical_entry(
        &self,
        target: &Arc<Target>,
        entry_index: usize,
        request_cancel: Option<&CancellationToken>,
        mut visit: impl FnMut(Logical<'_>) -> Result<()>,
    ) -> Result<()> {
        check_cancel(target, request_cancel)?;
        let bytes = self.dex_bytes(target, entry_index, request_cancel)?;
        let offsets =
            crate::dex::container::logical_offsets(&bytes).map_err(SessionError::invalid)?;
        let multiple = offsets.len() > 1;
        for (logical_index, offset) in offsets.into_iter().enumerate() {
            check_cancel(target, request_cancel)?;
            let data = crate::dex::container::logical_view(&bytes, offset)
                .map_err(SessionError::invalid)?;
            let base = &target.entries[entry_index].name;
            let dex_id = if multiple {
                format!("{base}!classes{}.dex", logical_index + 1)
            } else {
                base.clone()
            };
            visit(Logical {
                dex_id,
                entry_index,
                logical_index,
                data,
            })?;
        }
        Ok(())
    }

    fn for_each_logical(
        &self,
        target: &Arc<Target>,
        request_cancel: Option<&CancellationToken>,
        mut visit: impl FnMut(Logical<'_>) -> Result<()>,
    ) -> Result<()> {
        for &entry_index in &target.dex_entries {
            check_cancel(target, request_cancel)?;
            let bytes = self.dex_bytes(target, entry_index, request_cancel)?;
            let offsets =
                crate::dex::container::logical_offsets(&bytes).map_err(SessionError::invalid)?;
            let multiple = offsets.len() > 1;
            for (logical_index, offset) in offsets.into_iter().enumerate() {
                check_cancel(target, request_cancel)?;
                let data = crate::dex::container::logical_view(&bytes, offset)
                    .map_err(SessionError::invalid)?;
                let base = &target.entries[entry_index].name;
                let dex_id = if multiple {
                    format!("{base}!classes{}.dex", logical_index + 1)
                } else {
                    base.clone()
                };
                visit(Logical {
                    dex_id,
                    entry_index,
                    logical_index,
                    data,
                })?;
            }
        }
        Ok(())
    }
}

fn parallelizable_entry_bytes(bytes: impl IntoIterator<Item = usize>) -> usize {
    let (total, largest) = bytes
        .into_iter()
        .fold((0usize, 0usize), |(total, largest), bytes| {
            (total.saturating_add(bytes), largest.max(bytes))
        });
    total.saturating_sub(largest)
}

fn send_item_batch<T>(
    sender: &std::sync::mpsc::SyncSender<(usize, EntryEvent<T>)>,
    ordinal: usize,
    batch: &mut Vec<(T, usize)>,
    batch_bytes: &mut usize,
    row_gate: &RowGate,
    row_batch: usize,
    stop: &AtomicBool,
) -> bool {
    if batch.is_empty() {
        return true;
    }
    let rows = batch.len();
    let admitted = *batch_bytes;
    if !row_gate.acquire(ordinal, admitted, rows, stop) {
        return false;
    }
    let event = EntryEvent::Items(std::mem::take(batch), admitted);
    if send_event(sender, (ordinal, event), stop) {
        *batch_bytes = 0;
        batch.reserve(row_batch);
        true
    } else {
        row_gate.release(ordinal, admitted, rows);
        false
    }
}

fn send_event<T>(sender: &std::sync::mpsc::SyncSender<T>, mut event: T, stop: &AtomicBool) -> bool {
    loop {
        if stop.load(Ordering::Acquire) {
            return false;
        }
        match sender.try_send(event) {
            Ok(()) => return true,
            Err(std::sync::mpsc::TrySendError::Full(returned)) => {
                event = returned;
                std::thread::yield_now();
            }
            Err(std::sync::mpsc::TrySendError::Disconnected(_)) => return false,
        }
    }
}

fn recv_event<T>(
    receiver: &Receiver<T>,
    stop: &AtomicBool,
    target: &Target,
    request_cancel: Option<&CancellationToken>,
) -> Result<T> {
    loop {
        check_cancel(target, request_cancel)?;
        match receiver.recv_timeout(Duration::from_millis(10)) {
            Ok(event) => return Ok(event),
            Err(RecvTimeoutError::Timeout) => {
                if stop.load(Ordering::Acquire) {
                    return Err(SessionError::new("CANCELLED", "analysis was cancelled"));
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err(SessionError::new(
                    "INTERNAL",
                    "parallel analysis worker ended without a result",
                ));
            }
        }
    }
}

fn check_cancel(target: &Target, request_cancel: Option<&CancellationToken>) -> Result<()> {
    if target.cancel.is_cancelled() || request_cancel.is_some_and(CancellationToken::is_cancelled) {
        return Err(SessionError::new("CANCELLED", "analysis was cancelled"));
    }
    Ok(())
}

fn make_query(
    kind: FindRefsKind,
    value: Option<&str>,
    class: Option<&str>,
    fuzzy_class: bool,
) -> Result<Query> {
    let class = class
        .map(|class| {
            if fuzzy_class {
                Ok(ClassQuery::Fuzzy(crate::query::fuzzy_class_pattern(class)))
            } else {
                crate::query::format_class_name(class)
                    .map(ClassQuery::Exact)
                    .map_err(SessionError::invalid)
            }
        })
        .transpose()?;
    Ok(match kind {
        FindRefsKind::String => Query::String(required(value, "string value")?.to_owned()),
        FindRefsKind::Type => Query::Type(required(value, "type value")?.to_owned()),
        FindRefsKind::Method => Query::Method(MemberQuery {
            name: value.map(str::to_owned),
            class,
        }),
        FindRefsKind::Field => Query::Field(MemberQuery {
            name: value.map(str::to_owned),
            class,
        }),
    })
}

fn required<'a>(value: Option<&'a str>, name: &str) -> Result<&'a str> {
    value
        .filter(|value| !value.is_empty())
        .ok_or_else(|| SessionError::new("INVALID_INPUT", format!("{name} must not be empty")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dex::tests::const_string_fixture;
    use std::fs;

    fn fixture() -> (tempfile::TempDir, AnalysisSession, String) {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("fixture.dex"), const_string_fixture(3)).unwrap();
        let session = AnalysisSession::new(super::super::SessionConfig::for_roots(vec![
            root.path().to_owned(),
        ]))
        .unwrap();
        let id = session
            .open(root.path().join("fixture.dex"))
            .unwrap()
            .target_id;
        (root, session, id)
    }

    fn multi_dex_session(analysis_threads: usize) -> (tempfile::TempDir, AnalysisSession, String) {
        multi_dex_session_with(analysis_threads, usize::MAX)
    }

    fn multi_dex_session_with(
        analysis_threads: usize,
        max_result_items: usize,
    ) -> (tempfile::TempDir, AnalysisSession, String) {
        let root = tempfile::tempdir().unwrap();
        let first = const_string_fixture(2);
        let second = const_string_fixture(3);
        fs::write(
            root.path().join("fixture.apk"),
            crate::zip::tests::build_zip(&[
                ("classes.dex", &first, false),
                ("classes2.dex", &second, true),
            ]),
        )
        .unwrap();
        let mut config = super::super::SessionConfig::for_roots(vec![root.path().to_owned()]);
        config.analysis_threads = analysis_threads;
        config.max_result_items = max_result_items;
        config.force_parallel_entries = analysis_threads > 1;
        let session = AnalysisSession::new(config).unwrap();
        let id = session
            .open(root.path().join("fixture.apk"))
            .unwrap()
            .target_id;
        (root, session, id)
    }

    #[test]
    fn admission_measures_work_outside_the_largest_entry() {
        assert_eq!(parallelizable_entry_bytes([6 << 20, 512 << 10]), 512 << 10);
        assert_eq!(
            parallelizable_entry_bytes([6 << 20, 5 << 20, 1 << 20]),
            6 << 20
        );
        assert_eq!(parallelizable_entry_bytes([]), 0);
    }

    #[test]
    fn small_or_skewed_multi_dex_workload_uses_threshold_fallback_without_changing_results() {
        let (_serial_root, serial, serial_id) = multi_dex_session(1);
        let root = tempfile::tempdir().unwrap();
        let first = const_string_fixture(2);
        let second = const_string_fixture(3);
        fs::write(
            root.path().join("fixture.apk"),
            crate::zip::tests::build_zip(&[
                ("classes.dex", &first, false),
                ("classes2.dex", &second, true),
            ]),
        )
        .unwrap();
        let mut config = super::super::SessionConfig::for_roots(vec![root.path().to_owned()]);
        config.analysis_threads = 4;
        assert!(!config.force_parallel_entries);
        let sizes = [first.len(), second.len()];
        let parallelizable_tail =
            sizes.into_iter().sum::<usize>() - sizes.into_iter().max().unwrap();
        assert!(parallelizable_tail < 4 << 20);
        let thresholded = AnalysisSession::new(config).unwrap();
        let thresholded_id = thresholded
            .open(root.path().join("fixture.apk"))
            .unwrap()
            .target_id;
        assert_eq!(
            serde_json::to_value(thresholded.classes(&thresholded_id).unwrap()).unwrap(),
            serde_json::to_value(serial.classes(&serial_id).unwrap()).unwrap()
        );
        assert_eq!(
            serde_json::to_value(thresholded.strings(&thresholded_id).unwrap()).unwrap(),
            serde_json::to_value(serial.strings(&serial_id).unwrap()).unwrap()
        );
    }

    #[test]
    fn parallel_classes_and_strings_match_serial_order_and_cache_semantics() {
        let (_serial_root, serial, serial_id) = multi_dex_session(1);
        let serial_classes = serial.classes(&serial_id).unwrap();
        let serial_strings = serial.strings(&serial_id).unwrap();

        let (_parallel_root, parallel, parallel_id) = multi_dex_session(4);
        let parallel_classes = parallel.classes(&parallel_id).unwrap();
        let after_classes = parallel.status();
        let parallel_strings = parallel.strings(&parallel_id).unwrap();
        let after_strings = parallel.status();

        assert_eq!(
            serde_json::to_value(&parallel_classes).unwrap(),
            serde_json::to_value(&serial_classes).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&parallel_strings).unwrap(),
            serde_json::to_value(&serial_strings).unwrap()
        );
        assert_eq!(
            parallel_classes
                .items
                .iter()
                .map(|item| (item.entry_index, item.class_def_idx))
                .collect::<Vec<_>>(),
            [(0, 0), (0, 1), (1, 0), (1, 1), (1, 2)]
        );
        assert_eq!(after_classes.inflate_cache_loads, 1);
        assert_eq!(after_classes.inflate_cache_hits, 0);
        assert_eq!(after_strings.inflate_cache_loads, 1);
        assert_eq!(after_strings.inflate_cache_hits, 1);

        let serial_references = serial
            .findrefs(
                &serial_id,
                FindRefsKind::String,
                Some("Authorization"),
                None,
                false,
            )
            .unwrap();
        let parallel_references = parallel
            .findrefs(
                &parallel_id,
                FindRefsKind::String,
                Some("Authorization"),
                None,
                false,
            )
            .unwrap();
        assert_eq!(
            serde_json::to_value(parallel_references).unwrap(),
            serde_json::to_value(serial_references).unwrap()
        );
    }

    #[test]
    fn parallel_findrefs_respects_complete_result_budget() {
        let (_root, session, id) = multi_dex_session_with(4, 3);
        let error = session
            .findrefs(
                &id,
                FindRefsKind::String,
                Some("Authorization"),
                None,
                false,
            )
            .unwrap_err();
        assert_eq!(error.code, "RESOURCE_LIMIT");
    }

    #[test]
    fn parallel_batch_preserves_budget_priority_over_a_malformed_row() {
        let root = tempfile::tempdir().unwrap();
        let mut first = const_string_fixture(80);
        let classes_off = u32::from_le_bytes(first[0x64..0x68].try_into().unwrap()) as usize;
        first[classes_off + 64..classes_off + 68].copy_from_slice(&u32::MAX.to_le_bytes());
        let second = const_string_fixture(1);
        fs::write(
            root.path().join("fixture.apk"),
            crate::zip::tests::build_zip(&[
                ("classes.dex", &first, false),
                ("classes2.dex", &second, false),
            ]),
        )
        .unwrap();
        let mut config = super::super::SessionConfig::for_roots(vec![root.path().to_owned()]);
        config.analysis_threads = 4;
        config.max_result_items = 1;
        config.force_parallel_entries = true;
        let session = AnalysisSession::new(config).unwrap();
        let id = session
            .open(root.path().join("fixture.apk"))
            .unwrap()
            .target_id;
        assert_eq!(session.classes(&id).unwrap_err().code, "RESOURCE_LIMIT");
    }

    #[test]
    fn parallel_budget_wins_before_a_later_malformed_entry() {
        let root = tempfile::tempdir().unwrap();
        let first = const_string_fixture(2);
        let mut malformed = const_string_fixture(1);
        malformed[0x64..0x68].copy_from_slice(&u32::MAX.to_le_bytes());
        fs::write(
            root.path().join("fixture.apk"),
            crate::zip::tests::build_zip(&[
                ("classes.dex", &first, false),
                ("classes2.dex", &malformed, false),
            ]),
        )
        .unwrap();
        let mut config = super::super::SessionConfig::for_roots(vec![root.path().to_owned()]);
        config.analysis_threads = 4;
        config.max_result_items = 1;
        config.force_parallel_entries = true;
        let session = AnalysisSession::new(config).unwrap();
        let id = session
            .open(root.path().join("fixture.apk"))
            .unwrap()
            .target_id;
        assert_eq!(session.classes(&id).unwrap_err().code, "RESOURCE_LIMIT");
    }

    #[test]
    fn findrefs_scratch_limit_is_a_resource_limit() {
        let (_root, mut session, id) = multi_dex_session(1);
        session.config.max_result_bytes = 1;
        let error = session
            .findrefs(
                &id,
                FindRefsKind::String,
                Some("Authorization"),
                None,
                false,
            )
            .unwrap_err();
        assert_eq!(error.code, "RESOURCE_LIMIT");
    }

    #[test]
    fn parallel_classes_and_strings_observe_precancelled_requests() {
        let (_root, session, id) = multi_dex_session(4);
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert_eq!(
            session
                .classes_cancellable(&id, Some(&cancel))
                .unwrap_err()
                .code,
            "CANCELLED"
        );
        assert_eq!(
            session
                .strings_cancellable(&id, Some(&cancel))
                .unwrap_err()
                .code,
            "CANCELLED"
        );
    }

    #[test]
    fn getclass_normalizes_each_dex_041_logical_member() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("container.dex"),
            crate::dex::tests::dex041_container(&[1, 2]),
        )
        .unwrap();
        let session = AnalysisSession::new(super::super::SessionConfig::for_roots(vec![
            root.path().to_owned(),
        ]))
        .unwrap();
        let id = session
            .open(root.path().join("container.dex"))
            .unwrap()
            .target_id;
        let classes = session.classes(&id).unwrap();
        for wanted in ["0:0:0", "0:1:1"] {
            let class = classes
                .items
                .iter()
                .find(|item| item.class_id == wanted)
                .unwrap();
            let source = session.getclass(&id, Some(wanted), None).unwrap();
            assert_eq!(source.class_id, wanted);
            assert!(
                source.source.contains(
                    class
                        .dotted_name
                        .rsplit('.')
                        .next()
                        .expect("fixture class name")
                )
            );
        }
    }

    #[test]
    fn classes_are_complete_and_ids_select_source() {
        let (_root, session, id) = fixture();
        let classes = session.classes(&id).unwrap();
        assert_eq!(classes.items.len(), 3);
        assert_eq!(classes.items[0].descriptor, "LFixture0;");
        let result = session
            .getclass(&id, Some(&classes.items[0].class_id), None)
            .unwrap();
        assert!(
            result.source.contains("class Fixture0"),
            "{}",
            result.source
        );
    }

    #[test]
    fn strings_and_references_are_complete_and_structured() {
        let (_root, session, id) = fixture();
        let strings = session.strings(&id).unwrap();
        assert!(
            strings
                .items
                .iter()
                .any(|item| item.value == "Authorization")
        );
        let references = session
            .findrefs(
                &id,
                FindRefsKind::String,
                Some("Authorization"),
                None,
                false,
            )
            .unwrap();
        assert_eq!(references.items.len(), 3);
        assert_eq!(references.items[0].method_index, 0);
    }

    #[test]
    fn result_item_budget_stops_complete_datasets_atomically() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("fixture.dex"), const_string_fixture(3)).unwrap();
        let mut config = super::super::SessionConfig::for_roots(vec![root.path().to_owned()]);
        config.max_result_items = 2;
        let session = AnalysisSession::new(config).unwrap();
        let id = session
            .open(root.path().join("fixture.dex"))
            .unwrap()
            .target_id;
        let error = session.classes(&id).unwrap_err();
        assert_eq!(error.code, "RESOURCE_LIMIT");
        assert!(error.message.contains("streaming CLI"));
    }

    #[test]
    fn item_budget_stops_before_parsing_later_malformed_rows() {
        let root = tempfile::tempdir().unwrap();
        let mut dex = const_string_fixture(3);
        let classes_off = u32::from_le_bytes(dex[0x64..0x68].try_into().unwrap()) as usize;
        dex[classes_off + 64..classes_off + 68].copy_from_slice(&u32::MAX.to_le_bytes());
        fs::write(root.path().join("fixture.dex"), dex).unwrap();
        let mut config = super::super::SessionConfig::for_roots(vec![root.path().to_owned()]);
        config.max_result_items = 1;
        let session = AnalysisSession::new(config).unwrap();
        let id = session
            .open(root.path().join("fixture.dex"))
            .unwrap()
            .target_id;
        assert_eq!(session.classes(&id).unwrap_err().code, "RESOURCE_LIMIT");
    }

    #[test]
    fn result_byte_budget_rejects_complete_source() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("fixture.dex"), const_string_fixture(1)).unwrap();
        let mut config = super::super::SessionConfig::for_roots(vec![root.path().to_owned()]);
        config.max_result_bytes = 32;
        let session = AnalysisSession::new(config).unwrap();
        let id = session
            .open(root.path().join("fixture.dex"))
            .unwrap()
            .target_id;
        assert_eq!(
            session
                .getclass(&id, None, Some("Fixture0"))
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT"
        );
    }

    #[test]
    fn class_requests_rebuild_isolated_emitters() {
        let (_root, session, id) = fixture();
        let first = session
            .getclass(&id, None, Some("Fixture0"))
            .unwrap()
            .source;
        session.getclass(&id, None, Some("Fixture1")).unwrap();
        let again = session
            .getclass(&id, None, Some("Fixture0"))
            .unwrap()
            .source;
        assert_eq!(first, again);
        let status = session.status();
        assert_eq!(status.inflate_cache_entries, 0);
        assert_eq!(status.inflate_cache_loads, 0);
        assert_eq!(status.inflate_cache_hits, 0);
    }
}
