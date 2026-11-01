use crate::input::{MethodInput, RawPrintable};
use crate::p5_types::ArrayElement;
use crate::p6_variables::arrays::FreshArray;
use crate::pipeline::fresh_array::Candidate;
use crate::raw::instruction_layout;
use crate::substrate::owned::{TextBuffer, copy_units, reserve};
use crate::{EnvironmentId, MethodBudget, RequestError, VerifiedJavaMethod};
pub(crate) struct FreshArrayBody {
    pub proof: FreshArray,
    pub elements: Vec<ArrayElement>,
}
pub(crate) fn verify(
    input: &MethodInput<'_, '_>,
    candidate: Candidate<'_>,
    environment: EnvironmentId,
    budget: &mut MethodBudget<'_, '_>,
) -> Result<Option<VerifiedJavaMethod>, RequestError> {
    let proof = crate::p6_variables::arrays::prove(input, candidate, budget)?;
    let RawPrintable::Method(raw) = input.raw() else {
        return Err(RequestError::InternalInvariant(
            "fresh array lost raw method",
        ));
    };
    let flags = input.access_flags();
    if flags & !0x081f != 0
        || (flags & 7).count_ones() > 1
        || raw.annotations().method_range().is_some()
        || raw.annotations().parameter_range().is_some()
    {
        return Ok(None);
    }
    let original = raw.layout().original();
    let layout = instruction_layout(
        original.instructions(),
        proof.fill.payload,
        original.version(),
    )
    .map_err(crate::input::MethodError::Decode)?;
    let payload =
        crate::input::operands::array_payload(original.instructions(), proof.fill.payload, layout)
            .map_err(crate::input::MethodError::Decode)?;
    let mut elements = Vec::new();
    reserve(
        &mut elements,
        usize::try_from(proof.fill.count).map_err(|_| RequestError::PlatformCapacity)?,
        budget,
    )?;
    for value in payload {
        budget.charge_work(1)?;
        if value.width.0 != proof.allocation.kind.width() {
            return Err(RequestError::InternalInvariant(
                "certified array payload changed width",
            ));
        }
        let Some(element) = proof.allocation.kind.element(value.bits) else {
            return Err(RequestError::ArrayFillShape { pc: proof.fill.pc });
        };
        if element.bits() != value.bits || element.kind() != proof.allocation.kind {
            return Err(RequestError::InternalInvariant(
                "array literal changed original bits or component",
            ));
        }
        elements.push(element);
    }
    if elements.len()
        != usize::try_from(proof.fill.count).map_err(|_| RequestError::PlatformCapacity)?
    {
        return Err(RequestError::InternalInvariant(
            "array literal count changed",
        ));
    }
    let name = copy_units(input.declaration().name, budget)?;
    let Some(name) = super::calls::identifier_text(&name, budget)? else {
        return Ok(None);
    };
    let mut header = TextBuffer::new();
    for (bit, text) in [
        (1, "public "),
        (2, "private "),
        (4, "protected "),
        (8, "static "),
        (16, "final "),
        (0x800, "strictfp "),
    ] {
        if flags & bit != 0 {
            header.push(text, budget)?;
        }
    }
    header.push(proof.allocation.kind.spelling(), budget)?;
    header.push(" ", budget)?;
    header.push(&name, budget)?;
    header.push("()", budget)?;
    Ok(Some(VerifiedJavaMethod::fresh_array(
        header.finish(),
        FreshArrayBody { proof, elements },
        environment,
    )))
}
