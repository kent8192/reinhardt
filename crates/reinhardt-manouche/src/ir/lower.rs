//! Typed AST to IR lowering.

use crate::core::{TypedFormMacro, TypedPageMacro};
use crate::validator::TypedHeadMacro;

use super::{ComponentIR, FormIR, HeadIR};

/// Lowers a typed page AST to IR.
#[allow(clippy::unimplemented)] // Intentionally unsupported: page lowering uses direct codegen path instead of IR pipeline
pub fn lower_page(_typed: &TypedPageMacro) -> ComponentIR {
	unimplemented!("page lowering uses direct codegen path instead of IR pipeline")
}

/// Lowers a typed form AST to IR.
#[allow(clippy::unimplemented)] // Intentionally unsupported: form lowering uses direct codegen path instead of IR pipeline
pub fn lower_form(_typed: &TypedFormMacro) -> FormIR {
	unimplemented!("form lowering uses direct codegen path instead of IR pipeline")
}

/// Lowers a typed head AST to IR.
#[allow(clippy::unimplemented)] // Intentionally unsupported: head lowering uses direct codegen path instead of IR pipeline
pub fn lower_head(_typed: &TypedHeadMacro) -> HeadIR {
	unimplemented!("head lowering uses direct codegen path instead of IR pipeline")
}
