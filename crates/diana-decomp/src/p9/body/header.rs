//! Declarations, exception chains and bootstrap definitions.

use super::super::expressions::Expressions;
use super::super::{Origin, RenderError};
use crate::p7::*;

impl Expressions<'_, '_> {
    pub(super) fn locals(&mut self) -> Result<(), RenderError> {
        for l in self.code.locals() {
            if self.code.emission().hidden_local(l.id)
                || self.code.emission().allocation_local(l.id)
                || matches!(l.original.kind(), crate::p5::LocalKind::Parameter { .. })
            {
                // An allocated object's declaration stands at its allocation.
                continue;
            }
            self.out.indent()?;
            match self.code.emission().local_types()[l.id.0 as usize] {
                LocalSpelling::Named(t) => self.ty(t)?,
                LocalSpelling::Machine(t) => super::expressions::machine_type(self.out, t)?,
            }
            self.out.put(" ")?;
            self.local(l.id)?;
            self.out.put(";")?;
            if let Some(gap) = l.gap {
                self.out
                    .fmt(format_args!(" unavailable_storage<{gap:?}>;"))?;
            }
            self.out.put("\n")?;
        }
        for issue in self.code.issues() {
            self.out.indent()?;
            self.out.fmt(format_args!("diagnostic {issue:?};\n"))?;
        }
        Ok(())
    }

    /// The caught type of one clause, without its route target.
    pub(super) fn clause_type(&mut self, c: &Clause) -> Result<(), RenderError> {
        if let Some(catch) = &c.catch {
            match &catch.descriptor {
                Ok(t) => self.ty(*t)?,
                Err(e) => {
                    self.out
                        .fmt(format_args!("unavailable_catch<{}; ", catch.original.0))?;
                    self.out.quote(&e.detail)?;
                    self.out.put(">")?;
                }
            }
            if let Some(e) = &catch.use_issue {
                self.out.put(" [")?;
                self.out.quote(&e.detail)?;
                self.out.put("]")?;
            }
        } else {
            self.out.put("all")?;
        }
        Ok(())
    }

    pub(super) fn clause(&mut self, c: &Clause) -> Result<(), RenderError> {
        if let Some(catch) = &c.catch {
            match &catch.descriptor {
                Ok(t) => self.ty(*t)?,
                Err(e) => {
                    self.out
                        .fmt(format_args!("unavailable_catch<{}; ", catch.original.0))?;
                    self.out.quote(&e.detail)?;
                    self.out.put(">")?;
                }
            }
            if let Some(e) = &catch.use_issue {
                self.out.put(" [")?;
                self.out.quote(&e.detail)?;
                self.out.put("]")?;
            }
        } else {
            self.out.put("all")?;
        }
        self.out.put(" -> ")?;
        if let Some(target) = c.target {
            self.label_name(target)?;
        } else {
            self.out.put("propagate")?;
        }
        Ok(())
    }

    pub(super) fn exception_chains(&mut self) -> Result<(), RenderError> {
        for chain in self.code.chains() {
            if chain.clauses.is_empty() || !self.code.emission().show_chains()[chain.id.0 as usize]
            {
                continue;
            }
            self.out.indent()?;
            self.out
                .fmt(format_args!("exception_chain c{} {{ ", chain.id.0))?;
            for c in &chain.clauses {
                self.out.fmt(format_args!("{}: ", c.order))?;
                self.clause(c)?;
                self.out.put("; ")?;
            }
            self.out.put("}\n")?;
        }
        Ok(())
    }

    pub(super) fn bootstrap_definitions(&mut self) -> Result<(), RenderError> {
        for r in self.code.references() {
            if let Ok(ReferenceDeclaration::CallSite {
                name,
                prototype,
                bootstrap_member,
                arguments,
                handle_id,
                handle_kind,
                prototype_id,
                name_id,
                argument_count,
                ..
            }) = &r.declaration
            {
                self.out.indent()?;
                let a = self.out.position();
                self.out.fmt(format_args!(
                    "call_site r{} {:?} {{ name: ",
                    r.id.0, r.position.reference
                ))?;
                self.out.name(&name.units, name.form)?;
                self.out.put("; prototype: ")?;
                self.signature(prototype)?;
                self.out.fmt(format_args!("; name_id={}; prototype_id={}; bootstrap_handle={} kind={}; argument_count={}; bootstrap: ",name_id.0,prototype_id.0,handle_id.0,handle_kind,argument_count))?;
                self.reference(*bootstrap_member)?;
                self.out.put("; encoded_arguments: ")?;
                for value in arguments {
                    match value {
                        Bootstrap::EndContainer => self.out.put("end_container; ")?,
                        Bootstrap::Value {
                            name,
                            value,
                            tag,
                            value_arg,
                            ..
                        } => {
                            if let Some(name) = name {
                                self.out.units(name, true)?;
                                self.out.put(" = ")?;
                            }
                            match value {
                                BootstrapLiteral::Reference { declaration, .. } => {
                                    self.reference(*declaration)?
                                }
                                BootstrapLiteral::Annotation { type_use, count } => {
                                    self.out.put("annotation_begin<")?;
                                    self.ty(*type_use)?;
                                    self.out.fmt(format_args!("; count={count}>"))?;
                                }
                                _ => self.out.fmt(format_args!("{value:?}"))?,
                            }
                            self.out
                                .fmt(format_args!(" [tag=0x{tag:02x}; arg={value_arg}]; "))?;
                        }
                    }
                }
                self.out.put("; }\n")?;
                self.out.span(
                    a,
                    Origin::Reference {
                        method: self.method,
                        reference: r.id,
                    },
                )?;
            }
        }
        Ok(())
    }
}
