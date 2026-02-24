//! Typed AST to IR lowering.

use crate::core::{TypedFormMacro, TypedPageMacro};
use crate::validator::TypedHeadMacro;

use super::{ComponentIR, FormIR, HeadIR};

/// Lowers a typed page AST to IR.
#[allow(clippy::todo)] // Planned feature: page lowering in manouche pipeline
pub fn lower_page(_typed: &TypedPageMacro) -> ComponentIR {
	todo!("Implement page lowering")
}

/// Lowers a typed form AST to IR.
#[allow(clippy::todo)] // Planned feature: form lowering in manouche pipeline
pub fn lower_form(_typed: &TypedFormMacro) -> FormIR {
	todo!("Implement form lowering")
}

/// Lowers a typed head AST to IR.
#[allow(clippy::todo)] // Planned feature: head lowering in manouche pipeline
pub fn lower_head(_typed: &TypedHeadMacro) -> HeadIR {
	todo!("Implement head lowering")
}
