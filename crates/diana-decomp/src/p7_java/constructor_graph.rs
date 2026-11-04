use crate::input::MethodSource;
use crate::p4_types::entry_initialization::Link;
use crate::p4_types::initialization::ChainKind;
use crate::p5_variables::StorageBody;
use crate::substrate::owned::push;
use crate::{CodeUnitOffset, MethodId, RequestError, RunContext};

pub(crate) struct Proof<'m, 'i, 'a> {
    input: &'m MethodSource<'i, 'a>,
    entry: Option<(MethodId, CodeUnitOffset)>,
}
impl Proof<'_, '_, '_> {
    pub(crate) fn check(
        &self,
        input: &MethodSource<'_, '_>,
        body: &StorageBody<'_>,
        budget: &mut RunContext<'_>,
    ) -> Result<(), RequestError> {
        budget.checkpoint()?;
        let entry = body
            .constructor_entry
            .as_ref()
            .filter(|entry| entry.kind == ChainKind::This)
            .map(|entry| (entry.reference, entry.pc));
        if !std::ptr::eq(self.input, input) || self.entry != entry {
            return Err(RequestError::InternalInvariant(
                "constructor graph certificate changed selected method",
            ));
        }
        Ok(())
    }
}
enum State {
    Need(MethodId),
    Finished,
}
pub(crate) struct Graph<'m, 'i, 'a> {
    proof: Proof<'m, 'i, 'a>,
    seen: Vec<MethodId>,
    state: State,
}
impl<'m, 'i, 'a> Graph<'m, 'i, 'a> {
    pub(crate) fn new(
        _schedule: &crate::engine::SchedulePermit,
        input: &'m MethodSource<'i, 'a>,
        body: &StorageBody<'_>,
        budget: &mut RunContext<'_>,
    ) -> Result<Self, RequestError> {
        Self::new_impl(input, body, budget)
    }
    fn new_impl(
        input: &'m MethodSource<'i, 'a>,
        body: &StorageBody<'_>,
        budget: &mut RunContext<'_>,
    ) -> Result<Self, RequestError> {
        let entry = body
            .constructor_entry
            .as_ref()
            .filter(|entry| entry.kind == ChainKind::This)
            .map(|entry| (entry.reference, entry.pc));
        let mut seen = Vec::new();
        let state = match entry {
            Some((target, _)) => {
                push(&mut seen, input.method_id(), budget)?;
                State::Need(target)
            }
            None => State::Finished,
        };
        Ok(Self {
            proof: Proof { input, entry },
            seen,
            state,
        })
    }
    pub(crate) fn unavailable(&self) -> RequestError {
        match self.proof.entry {
            Some((_, pc)) => RequestError::InitializationUnavailable { pc },
            None => RequestError::InternalInvariant(
                "non-delegating constructor requested graph evidence",
            ),
        }
    }
    pub(crate) fn pending(
        &self,
        budget: &mut RunContext<'_>,
    ) -> Result<Option<MethodId>, RequestError> {
        budget.checkpoint()?;
        let State::Need(target) = self.state else {
            return Ok(None);
        };
        for &previous in &self.seen {
            budget.checkpoint()?;
            if previous == target {
                return Err(self.unavailable());
            }
        }
        Ok(Some(target))
    }
    pub(crate) fn accept(
        &mut self,
        link: &Link,
        budget: &mut RunContext<'_>,
    ) -> Result<(), RequestError> {
        let Some(source) = self.pending(budget)? else {
            return Err(RequestError::InternalInvariant(
                "completed constructor graph received another link",
            ));
        };
        if link.method() != source {
            return Err(RequestError::InternalInvariant(
                "constructor link changed original source method",
            ));
        }
        push(&mut self.seen, source, budget)?;
        self.state = match link.kind() {
            ChainKind::This => State::Need(link.target()),
            ChainKind::Super => State::Finished,
        };
        Ok(())
    }
    pub(crate) fn finish(self) -> Result<Proof<'m, 'i, 'a>, RequestError> {
        if !matches!(self.state, State::Finished) {
            return Err(RequestError::InternalInvariant(
                "constructor graph sealed with pending target",
            ));
        }
        Ok(self.proof)
    }
}
