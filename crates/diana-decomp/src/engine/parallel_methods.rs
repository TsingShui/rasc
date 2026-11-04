use super::{DiagnosticLog, Engine, MemberInputs, add_diagnostic};
use crate::input::ProgramIndex;
use crate::input::catalog::{MemberEntry, MemberSelection};
use crate::substrate::owned::reserve;
use crate::{ClassKey, DiagnosticId, MethodId, MethodKey, MethodOutput, RequestError, RunContext};
use rayon::prelude::*;

impl<'a> Engine<'a> {
    pub(super) fn parallel_members(
        &self,
        index: &ProgramIndex<'a>,
        class: ClassKey,
        emission: &crate::p7_java::member_emission::Plan,
        inputs: MemberInputs<'_>,
        diagnostics: &mut DiagnosticLog,
        control: &mut RunContext<'_>,
    ) -> Result<Option<Vec<MethodOutput>>, RequestError> {
        if self.method_jobs == 1 {
            return Ok(None);
        }
        if !control.supports_parallel() {
            return Ok(None);
        }
        let mut methods: Vec<MethodId> = Vec::new();
        let visit = index
            .class_with(class.class_def, control)?
            .visit_members_with(
                control,
                MemberSelection::AllHeaders,
                |entry, control| -> Result<(), RequestError> {
                    if let MemberEntry::MethodHeader { method_idx, .. } = entry {
                        reserve(&mut methods, 1, control)?;
                        methods.push(method_idx);
                    }
                    Ok(())
                },
            );
        if let Err(error) = visit {
            // Keep the serial visitation path's local-boundary behavior. No
            // worker has started and no partial member set was published.
            if matches!(
                error.kind(),
                crate::FailureKind::Malformed | crate::FailureKind::Unsupported
            ) {
                return Ok(None);
            }
            return Err(error);
        }
        if methods.len() < 2 {
            return Ok(None);
        }
        let pool = self.method_pool.get_or_init(|| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(self.method_jobs)
                .build()
                .map_err(|error| error.to_string())
        });
        let pool = pool.as_ref().map_err(|_| RequestError::PlatformCapacity)?;
        // Prepare the result slots before workers start. Per-member owned data
        // and diagnostics are charged by each worker against the same request.
        let mut results = Vec::new();
        reserve(&mut results, methods.len(), control)?;
        // Include allocations performed during header enumeration and slot setup.
        let parallel = control
            .parallel_control()?
            .ok_or(RequestError::InternalInvariant(
                "parallel callback disappeared",
            ))?;
        pool.install(|| {
            methods
                .par_iter()
                .map(|&method| {
                    let mut control = parallel.worker();
                    let mut diagnostics = DiagnosticLog {
                        values: Vec::new(),
                        primary: None,
                    };
                    let result = self.member_with_generated(
                        index,
                        MethodKey { class, method },
                        &mut diagnostics,
                        emission,
                        inputs,
                        &mut control,
                    );
                    (result, diagnostics)
                })
                .collect_into_vec(&mut results)
        });
        control.finish_parallel(&parallel)?;
        let mut members = Vec::new();
        reserve(&mut members, results.len(), control)?;
        for (result, local) in results {
            let mut member = result?;
            let offset = diagnostics.values.len();
            for diagnostic in local.values {
                add_diagnostic(diagnostics, diagnostic, control)?;
            }
            match &mut member {
                MethodOutput::Debug { reason, .. } | MethodOutput::Unavailable { reason, .. } => {
                    *reason = DiagnosticId(
                        offset
                            .checked_add(reason.0)
                            .ok_or(RequestError::PlatformCapacity)?,
                    );
                }
                _ => {}
            }
            members.push(member);
        }
        Ok(Some(members))
    }
}
