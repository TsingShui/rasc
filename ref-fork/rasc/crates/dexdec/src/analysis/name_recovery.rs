use std::collections::{BTreeMap, BTreeSet};

use crate::ir::{
    analysis::{VariableEdgeKind, VariableSemanticGraph},
    ArgType, InsnType,
};

pub(crate) struct RelationalNameInference {
    names: BTreeMap<u32, String>,
}

impl RelationalNameInference {
    pub(crate) fn analyze(graph: &VariableSemanticGraph) -> Self {
        let names = graph
            .variables()
            .filter(|variable| variable.ty() == &ArgType::BOOLEAN)
            .filter_map(|variable| {
                let names = graph
                    .edges_for(variable.identity())
                    .filter(|edge| matches!(edge.kind(), VariableEdgeKind::Definition))
                    .filter_map(|edge| edge.operation().and_then(|id| graph.operation(id)))
                    .filter_map(Self::operation_name)
                    .collect::<BTreeSet<_>>();
                let mut names = names.into_iter();
                let name = names.next()?;
                names
                    .next()
                    .is_none()
                    .then_some((variable.identity(), name))
            })
            .collect();
        Self { names }
    }

    pub(crate) fn name(&self, variable: u32) -> Option<&str> {
        self.names.get(&variable).map(String::as_str)
    }

    fn operation_name(operation: &crate::ir::analysis::OperationNode) -> Option<String> {
        let crate::ir::MemberReference::Method(method) = operation.reference()? else {
            return None;
        };
        if operation.kind() != InsnType::Invoke
            || method.name != "equals"
            || method.descriptor.return_type != ArgType::BOOLEAN
        {
            return None;
        }
        let concepts = operation
            .operands()
            .iter()
            .filter_map(|operand| Self::concept(operand.reference()?))
            .collect::<BTreeSet<_>>();
        let mut concepts = concepts.into_iter();
        let concept = concepts.next()?;
        if concepts.next().is_some() {
            return None;
        }
        Some(format!("same{}", Self::upper_camel(&concept)))
    }

    fn concept(reference: &crate::ir::MemberReference) -> Option<String> {
        let name = match reference {
            crate::ir::MemberReference::Method(method) => method.name.as_str(),
            crate::ir::MemberReference::Field(field) => field.name.as_str(),
        };
        if name.starts_with('<')
            || name.contains('$')
            || name
                .chars()
                .filter(|character| character.is_alphanumeric())
                .count()
                < 3
        {
            return None;
        }
        let concept = name.strip_prefix("get").filter(|suffix| {
            suffix
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_uppercase())
        });
        Some(concept.unwrap_or(name).to_owned())
    }

    fn upper_camel(value: &str) -> String {
        let mut characters = value.chars();
        let Some(first) = characters.next() else {
            return String::new();
        };
        first.to_uppercase().chain(characters).collect()
    }
}
