use crate::context::Context;
use crate::input::catalog::{MemberEntry, MemberSelection};
use crate::{ClassKey, RequestError, RunContext};

pub(crate) fn collect(
    permit: &crate::engine::SchedulePermit,
    index: &crate::input::ProgramIndex<'_>,
    key: ClassKey,
    catalog: &Context,
    control: &mut RunContext<'_>,
) -> Result<crate::p7_java::member_emission::Plan, RequestError> {
    let enumeration = (|| -> Result<_, RequestError> {
        let Some(declaration) =
            crate::p4_types::declarations::enums::collect(permit, index, key, catalog, control)?
        else {
            return Ok(None);
        };
        crate::p7_java::enums::verify(permit, index, declaration, control)
    })();
    match enumeration {
        Ok(Some(proof)) => return Ok(crate::p7_java::member_emission::Plan::with_enum(proof)),
        Ok(None) => {}
        Err(error)
            if matches!(
                error.kind(),
                crate::FailureKind::Malformed | crate::FailureKind::Unsupported
            ) =>
        {
            return Ok(crate::p7_java::member_emission::Plan::rejected(error));
        }
        Err(error) => return Err(error),
    }
    let mut plan = crate::p7_java::member_emission::Plan::default();
    index
        .class_with(key.class_def, control)?
        .visit_members_with(control, MemberSelection::AllHeaders, |entry, control| {
            let MemberEntry::MethodHeader {
                method_idx,
                access_flags,
                ..
            } = entry
            else {
                return Ok(());
            };
            if !crate::substrate::flags::bridge(access_flags) {
                return Ok(());
            }
            let result = (|| -> Result<Option<crate::p7_java::GeneratedBridge>, RequestError> {
                let budget = &mut *control;
                let input = index
                    .method_source_at(key.class_def, method_idx, budget)?
                    .decode(budget)?;
                let Some(target) =
                    crate::p7_java::member_emission::candidate(permit, &input, budget)?
                else {
                    return Ok(None);
                };
                let Some(evidence) = crate::p4_types::declarations::overrides::collect(
                    permit, &input, target, catalog, budget,
                )?
                else {
                    return Ok(None);
                };
                crate::p7_java::member_emission::verify(permit, &input, &evidence, budget)
            })();
            match result {
                Ok(Some(bridge)) => plan.add(bridge, control)?,
                Ok(None) => {}
                Err(error)
                    if matches!(
                        error.kind(),
                        crate::FailureKind::Malformed | crate::FailureKind::Unsupported
                    ) => {}
                Err(error) => return Err(error),
            }
            Ok(())
        })?;
    Ok(plan)
}
