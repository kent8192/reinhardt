//! AST-based page! macro formatter implementation.
//!
//! This module provides formatting for `page!` macro DSL using proper AST parsing.
//! Unlike the text-based approach, this implementation:
//!
//! - Uses `syn::parse_file()` to parse the entire Rust source file
//! - Uses `syn::visit` to accurately detect `page!` macro invocations
//! - Ignores content in comments and strings (guaranteed by AST)
//! - Uses `reinhardt-pages-ast` for parsing the macro DSL
//!
//! ## Architecture
//!
//! ```mermaid
//! flowchart TB
//!     A["Rust source file"] --> B["syn::parse_file()<br/>Parse entire file to AST"]
//!     B --> C["PageMacroVisitor<br/>Walk AST to find page! macros"]
//!     C --> D["reinhardt_pages::ast::PageMacro<br/>Parse macro tokens to DSL AST"]
//!     D --> E["format_macro()<br/>Generate formatted code from AST"]
//!     E --> F["replace by span<br/>Replace original text"]
//!     F --> G["Formatted source file"]
//! ```

use quote::ToTokens;
use regex::Regex;
use reinhardt_pages::ast::{
	PageAttr, PageBody, PageComponent, PageElement, PageElse, PageEvent, PageExpression, PageFor,
	PageIf, PageMacro, PageNode, PageParam, PageText,
};
use std::path::PathBuf;
use std::process::Command;
use std::sync::LazyLock;
use syn::visit::Visit;
use syn::{ExprMacro, Macro, parse_file};

/// Reason why formatting was skipped for a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SkipReason {
	/// File-wide ignore-all marker detected
	FileWideMarker,
	/// No page! macro found in file
	NoPageMacro,
	/// All page! macros were individually ignored
	AllMacrosIgnored,
}

impl std::fmt::Display for SkipReason {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			SkipReason::FileWideMarker => write!(f, "file-wide ignore marker"),
			SkipReason::NoPageMacro => write!(f, "no page! macro"),
			SkipReason::AllMacrosIgnored => write!(f, "all macros ignored"),
		}
	}
}

/// Options to pass to rustfmt.
///
/// These options mirror rustfmt's command-line arguments and allow
/// customizing formatting behavior.
#[derive(Clone, Debug, Default)]
pub(crate) struct RustfmtOptions {
	/// Path to rustfmt.toml configuration file
	pub config_path: Option<PathBuf>,
	/// Rust edition to use (e.g., "2021", "2024")
	pub edition: Option<String>,
	/// Style edition to use
	pub style_edition: Option<String>,
	/// Inline config options (e.g., "max_width=120,hard_tabs=false")
	pub config: Option<String>,
	/// Color output setting (e.g., "auto", "always", "never")
	pub color: Option<String>,
}

impl RustfmtOptions {
	/// Apply these options to a rustfmt Command.
	pub(crate) fn apply_to_command(&self, cmd: &mut Command) {
		if let Some(ref path) = self.config_path {
			cmd.arg("--config-path").arg(path);
		}
		if let Some(ref edition) = self.edition {
			cmd.arg("--edition").arg(edition);
		}
		if let Some(ref style_edition) = self.style_edition {
			cmd.arg("--style-edition").arg(style_edition);
		}
		if let Some(ref config) = self.config {
			cmd.arg("--config").arg(config);
		}
		if let Some(ref color) = self.color {
			cmd.arg("--color").arg(color);
		}
	}
}

/// Result of formatting operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FormatResult {
	/// Formatted content
	pub content: String,
	/// Whether the file contains page! macros
	pub contains_page_macro: bool,
	/// If set, formatting was skipped for this reason
	pub skipped: Option<SkipReason>,
}

/// Information about a detected page! macro invocation.
#[derive(Debug)]
struct MacroInfo {
	/// Start byte offset in the source
	start: usize,
	/// End byte offset in the source
	end: usize,
	/// The macro's tokens (content inside page!(...))
	tokens: proc_macro2::TokenStream,
	/// Whether this macro should be skipped during formatting
	should_skip: bool,
}

/// Backup information for a protected page! macro.
///
/// Used during the protect/restore cycle to preserve page! macros
/// while rustfmt processes the surrounding Rust code.
#[derive(Debug, Clone)]
pub(crate) struct PageMacroBackup {
	/// Unique identifier for this macro (used in placeholder)
	pub id: usize,
	/// Original page! macro text (including "page!(...)")
	pub original: String,
}

/// Result of protecting page! macros in source code.
#[derive(Debug)]
pub(crate) struct ProtectResult {
	/// Source code with page! macros replaced by placeholders
	pub protected_content: String,
	/// Backup information for each replaced macro
	pub backups: Vec<PageMacroBackup>,
}

/// Visitor that walks the AST to find page! macro invocations.
struct PageMacroVisitor<'a> {
	/// Collected macro information
	macros: Vec<MacroInfo>,
	/// Original source code for offset calculation
	source: &'a str,
}

impl<'a> PageMacroVisitor<'a> {
	fn new(source: &'a str) -> Self {
		Self {
			macros: Vec::new(),
			source,
		}
	}

	/// Extract macro info from a Macro node.
	fn extract_macro_info(&mut self, mac: &Macro) {
		if mac.path.is_ident("page") {
			// Get span information
			// Note: proc_macro2::Span in non-procedural-macro context doesn't
			// give us byte offsets directly. We need to find the macro in source.
			let tokens_str = mac.tokens.to_string();

			// Find this macro in the source by searching for "page!("
			// We use the token stream content to verify we found the right one
			if let Some(info) = self.find_macro_in_source(&tokens_str) {
				self.macros.push(info);
			}
		}
	}

	/// Find the page! macro in source and return its position info.
	fn find_macro_in_source(&self, _tokens_content: &str) -> Option<MacroInfo> {
		// This is a simplified approach - we search for "page!(" patterns
		// and verify by comparing token content
		let pattern = "page!(";
		let mut search_start = 0;

		// Skip already found macros
		for found in &self.macros {
			if found.end > search_start {
				search_start = found.end;
			}
		}

		while let Some(pos) = self.source[search_start..].find(pattern) {
			let abs_start = search_start + pos;
			let content_start = abs_start + pattern.len();

			// Find matching closing paren
			if let Some(end_pos) = find_matching_paren(self.source, content_start) {
				let macro_content = &self.source[content_start..end_pos];

				// Parse the content to get tokens
				if let Ok(tokens) = syn::parse_str::<proc_macro2::TokenStream>(macro_content) {
					return Some(MacroInfo {
						start: abs_start,
						end: end_pos + 1, // Include closing paren
						tokens,
						should_skip: false,
					});
				}
			}

			search_start = abs_start + 1;
		}

		None
	}
}

impl<'ast, 'a> Visit<'ast> for PageMacroVisitor<'a> {
	fn visit_expr_macro(&mut self, expr: &'ast ExprMacro) {
		self.extract_macro_info(&expr.mac);
		syn::visit::visit_expr_macro(self, expr);
	}

	fn visit_macro(&mut self, mac: &'ast Macro) {
		self.extract_macro_info(mac);
		syn::visit::visit_macro(self, mac);
	}
}

/// Find the matching closing parenthesis, handling strings and nested parens.
///
/// Uses char_indices() to properly handle UTF-8 multi-byte characters.
fn find_matching_paren(source: &str, start: usize) -> Option<usize> {
	let substring = &source[start..];
	let mut depth = 1;
	let mut in_string = false;
	let mut in_char = false;
	let mut escape_next = false;
	let chars: Vec<(usize, char)> = substring.char_indices().collect();
	let mut i = 0;

	while i < chars.len() {
		let (offset, ch) = chars[i];

		if escape_next {
			escape_next = false;
			i += 1;
			continue;
		}

		if in_string {
			match ch {
				'\\' => escape_next = true,
				'"' => in_string = false,
				_ => {}
			}
			i += 1;
			continue;
		}

		if in_char {
			match ch {
				'\\' => escape_next = true,
				'\'' => in_char = false,
				_ => {}
			}
			i += 1;
			continue;
		}

		match ch {
			'"' => {
				// Check for raw strings: r#"..."# or r"..."
				// Look back to see if preceded by 'r' and optional '#'s
				let raw_start = detect_raw_string_start(substring, offset);
				if let Some(hash_count) = raw_start {
					// Skip raw string content until closing "###
					if let Some(end_offset) = skip_raw_string(substring, offset + 1, hash_count) {
						// Find the index in chars that corresponds to end_offset
						while i < chars.len() && chars[i].0 < end_offset {
							i += 1;
						}
						i += 1; // skip past end
						continue;
					}
				}
				in_string = true;
			}
			'\'' => {
				// Distinguish char literal from lifetime annotation:
				// Char literal: 'a', '\n', '\\'
				// Lifetime: 'a (letter not followed by closing quote in char-literal pattern)
				if is_char_literal(&chars, i) {
					in_char = true;
				}
				// Otherwise it's a lifetime, just skip the tick
			}
			'(' => depth += 1,
			')' => {
				depth -= 1;
				if depth == 0 {
					return Some(start + offset);
				}
			}
			_ => {}
		}
		i += 1;
	}

	None
}

/// Detect if a '"' at the given offset is the start of a raw string.
/// Returns Some(hash_count) if so (0 for r"...", 1 for r#"..."#, etc.).
fn detect_raw_string_start(s: &str, quote_offset: usize) -> Option<usize> {
	// Walk backwards from the quote to find r followed by optional #s
	let before = &s[..quote_offset];
	let trimmed = before.trim_end_matches('#');
	let hash_count = before.len() - trimmed.len();
	if trimmed.ends_with('r') {
		// Verify the 'r' is not part of an identifier
		let r_pos = trimmed.len() - 1;
		if r_pos == 0 || !before.as_bytes()[r_pos - 1].is_ascii_alphanumeric() {
			return Some(hash_count);
		}
	}
	None
}

/// Skip past the contents of a raw string starting after the opening '"'.
/// Returns the byte offset just past the closing '"' + hashes.
fn skip_raw_string(s: &str, start_after_quote: usize, hash_count: usize) -> Option<usize> {
	let closing_pattern: String = std::iter::once('"')
		.chain(std::iter::repeat_n('#', hash_count))
		.collect();
	s[start_after_quote..]
		.find(&closing_pattern)
		.map(|pos| start_after_quote + pos + closing_pattern.len())
}

/// Check if a `'\''` at `chars\[idx\]` starts a char literal (not a lifetime).
/// A char literal has the pattern: 'x' or '\x' or '\xx'
fn is_char_literal(chars: &[(usize, char)], idx: usize) -> bool {
	// After the opening quote, check if we see a closing quote pattern
	let remaining = &chars[idx + 1..];

	if remaining.is_empty() {
		return false;
	}

	// Pattern: '\...' (escaped char literal)
	if remaining[0].1 == '\\' {
		// Look for closing quote within the next few chars
		for item in remaining.iter().take(remaining.len().min(5)).skip(2) {
			if item.1 == '\'' {
				return true;
			}
		}
		return false;
	}

	// Pattern: 'x' (single char literal) - must have closing quote at position +2
	if remaining.len() >= 2 && remaining[1].1 == '\'' {
		return true;
	}

	// Otherwise, it's a lifetime ('a in type position, no closing quote)
	false
}

/// Maximum recursion depth for formatting nested nodes.
///
/// Prevents stack overflow from deeply nested or maliciously crafted
/// page! macro content. 128 levels is far more than any realistic
/// template would need.
const MAX_FORMAT_DEPTH: usize = 128;

/// AST-based page! macro formatter.
pub(crate) struct AstPageFormatter {
	/// Indentation string (tab by default)
	indent: String,
	/// Options to pass to rustfmt
	rustfmt_options: RustfmtOptions,
}

impl Default for AstPageFormatter {
	fn default() -> Self {
		Self::new()
	}
}

impl AstPageFormatter {
	/// Create a new formatter with default settings.
	pub(crate) fn new() -> Self {
		Self {
			indent: "\t".to_string(),
			rustfmt_options: RustfmtOptions::default(),
		}
	}

	/// Create a new formatter with the specified rustfmt options.
	#[allow(dead_code)] // Reserved for future use when full options support is needed
	pub(crate) fn with_options(rustfmt_options: RustfmtOptions) -> Self {
		Self {
			indent: "\t".to_string(),
			rustfmt_options,
		}
	}

	/// Create a new formatter with a specific config path.
	pub(crate) fn with_config(config_path: PathBuf) -> Self {
		Self {
			indent: "\t".to_string(),
			rustfmt_options: RustfmtOptions {
				config_path: Some(config_path),
				..Default::default()
			},
		}
	}

	/// Calculate the base indentation level for a macro at the given position.
	///
	/// Returns the number of tabs from the start of the line to the macro position.
	fn calculate_base_indent(content: &str, macro_start: usize) -> usize {
		// Find the start of the line containing the macro
		let line_start = content[..macro_start]
			.rfind('\n')
			.map(|pos| pos + 1)
			.unwrap_or(0);

		// Count tabs in the indentation
		let indent_str = &content[line_start..macro_start];
		indent_str.chars().filter(|&c| c == '\t').count()
	}

	/// Format the content of a Rust source file.
	///
	/// Uses AST parsing for accurate macro detection. Falls back to returning
	/// the original content if parsing fails.
	pub(crate) fn format(&self, content: &str) -> Result<FormatResult, String> {
		// Safety check FIRST: If no page! pattern exists, return unchanged
		if !content.contains("page!(") {
			return Ok(FormatResult {
				content: content.to_string(),
				contains_page_macro: false,
				skipped: Some(SkipReason::NoPageMacro),
			});
		}

		// Then check for file-wide ignore marker
		if self.has_ignore_all_marker(content) {
			return Ok(FormatResult {
				content: content.to_string(),
				contains_page_macro: true, // Contains page! but ignored
				skipped: Some(SkipReason::FileWideMarker),
			});
		}

		// Collect all page! macro locations
		let macros = self.find_page_macros(content)?;

		if macros.is_empty() {
			// Safety check passed but no actual macros found (e.g., in comments)
			return Ok(FormatResult {
				content: content.to_string(),
				contains_page_macro: false,
				skipped: Some(SkipReason::NoPageMacro),
			});
		}

		// Sort macros by position (they should already be in order)
		let mut macros = macros;
		macros.sort_by_key(|m| m.start);

		// Apply ignore markers to determine which macros to skip
		self.apply_ignore_markers(content, &mut macros);

		// Check if all macros are individually ignored
		if macros.iter().all(|m| m.should_skip) {
			return Ok(FormatResult {
				content: content.to_string(),
				contains_page_macro: true,
				skipped: Some(SkipReason::AllMacrosIgnored),
			});
		}

		// Build result by replacing each macro
		let mut result = String::with_capacity(content.len() * 2);
		let mut last_end = 0;

		for macro_info in &macros {
			// Skip if marked for ignore
			if macro_info.should_skip {
				// Copy the original macro as-is
				result.push_str(&content[last_end..macro_info.end]);
				last_end = macro_info.end;
				continue;
			}

			// Copy content before this macro
			result.push_str(&content[last_end..macro_info.start]);

			// Calculate base indentation for this macro
			let base_indent = Self::calculate_base_indent(content, macro_info.start);

			// Try to parse and format the macro
			match self.format_macro_tokens(&macro_info.tokens, base_indent) {
				Ok(formatted) => {
					result.push_str("page!(");
					result.push_str(&formatted);
					result.push(')');
				}
				Err(_) => {
					// If formatting fails, keep original
					result.push_str(&content[macro_info.start..macro_info.end]);
				}
			}

			last_end = macro_info.end;
		}

		// Copy remaining content
		result.push_str(&content[last_end..]);

		Ok(FormatResult {
			content: result,
			contains_page_macro: true,
			skipped: None,
		})
	}

	/// Find all page! macros in the source.
	fn find_page_macros(&self, content: &str) -> Result<Vec<MacroInfo>, String> {
		// Try to parse as a complete Rust file first
		match parse_file(content) {
			Ok(file) => {
				let mut visitor = PageMacroVisitor::new(content);
				visitor.visit_file(&file);
				Ok(visitor.macros)
			}
			Err(_) => {
				// If file parsing fails, fall back to text-based detection
				self.find_page_macros_text_based(content)
			}
		}
	}

	/// Text-based fallback for finding page! macros.
	fn find_page_macros_text_based(&self, content: &str) -> Result<Vec<MacroInfo>, String> {
		let mut macros = Vec::new();
		let pattern = "page!(";
		let mut search_start = 0;

		while let Some(pos) = content[search_start..].find(pattern) {
			let abs_start = search_start + pos;

			// Check if we're in a comment or string
			if self.is_in_comment_or_string(content, abs_start) {
				search_start = abs_start + 1;
				continue;
			}

			let content_start = abs_start + pattern.len();

			if let Some(end_pos) = find_matching_paren(content, content_start) {
				let macro_content = &content[content_start..end_pos];

				if let Ok(tokens) = syn::parse_str::<proc_macro2::TokenStream>(macro_content) {
					macros.push(MacroInfo {
						start: abs_start,
						end: end_pos + 1,
						tokens,
						should_skip: false,
					});
				}

				search_start = end_pos + 1;
			} else {
				search_start = abs_start + 1;
			}
		}

		Ok(macros)
	}

	/// Check if a position is inside a comment or string literal.
	///
	/// Uses char_indices() to properly handle UTF-8 multi-byte characters.
	fn is_in_comment_or_string(&self, content: &str, pos: usize) -> bool {
		let mut chars = content.char_indices().peekable();
		let mut in_string = false;
		let mut in_line_comment = false;
		let mut in_block_comment = false;
		let mut escape_next = false;

		while let Some((offset, ch)) = chars.next() {
			if offset >= pos {
				break;
			}

			if escape_next {
				escape_next = false;
				continue;
			}

			// Check for two-character sequences
			if !in_string
				&& !in_block_comment
				&& ch == '/' && let Some(&(_, next_ch)) = chars.peek()
			{
				if next_ch == '/' {
					in_line_comment = true;
					chars.next(); // consume second '/'
					continue;
				} else if next_ch == '*' {
					in_block_comment = true;
					chars.next(); // consume '*'
					continue;
				}
			}

			// Check for end of line comment
			if in_line_comment && ch == '\n' {
				in_line_comment = false;
				continue;
			}

			// Check for end of block comment
			if in_block_comment
				&& ch == '*' && let Some(&(_, next_ch)) = chars.peek()
				&& next_ch == '/'
			{
				in_block_comment = false;
				chars.next(); // consume '/'
				continue;
			}

			// Handle strings
			if !in_line_comment && !in_block_comment {
				match ch {
					'\\' if in_string => escape_next = true,
					'"' => in_string = !in_string,
					_ => {}
				}
			}
		}

		in_string || in_line_comment || in_block_comment
	}

	/// Format macro tokens to formatted string.
	fn format_macro_tokens(
		&self,
		tokens: &proc_macro2::TokenStream,
		base_indent: usize,
	) -> Result<String, String> {
		// Parse tokens as PageMacro
		let page_macro: PageMacro =
			syn::parse2(tokens.clone()).map_err(|e| format!("Parse error: {}", e))?;

		// Format the macro
		self.format_page_macro(&page_macro, base_indent)
	}

	/// Check if a page macro body is simple and can be formatted on a single line.
	fn is_simple_body(body: &PageBody) -> bool {
		// Simple if it has exactly one element with no attributes, events, or children
		if body.nodes.len() == 1
			&& let PageNode::Element(elem) = &body.nodes[0]
		{
			return elem.attrs.is_empty() && elem.events.is_empty() && elem.children.is_empty();
		}
		false
	}

	/// Format a PageMacro AST to string.
	fn format_page_macro(
		&self,
		macro_ast: &PageMacro,
		base_indent: usize,
	) -> Result<String, String> {
		let mut output = String::new();

		// Format closure parameters
		self.format_params(&mut output, &macro_ast.params);

		// Check if body is simple enough for single-line format
		if Self::is_simple_body(&macro_ast.body) {
			// Single-line format: || { div {} }
			output.push_str(" { ");
			if let PageNode::Element(elem) = &macro_ast.body.nodes[0] {
				output.push_str(&elem.tag.to_string());
				output.push_str(" {}");
			}
			output.push_str(" }");
		} else {
			// Multi-line format
			output.push_str(" {\n");
			self.format_body(&mut output, &macro_ast.body, base_indent + 1, 0);
			output.push_str(&self.make_indent(base_indent));
			output.push('}');
		}

		Ok(output)
	}

	/// Format closure parameters: |param: Type, ...|
	fn format_params(&self, output: &mut String, params: &[PageParam]) {
		output.push('|');
		for (i, param) in params.iter().enumerate() {
			if i > 0 {
				output.push_str(", ");
			}

			let param_name = param.name.to_string();
			let ty_str = param.ty.to_token_stream().to_string();

			output.push_str(&param_name);

			// Skip type annotation for underscore-only parameters with type inference
			// to preserve |_| format instead of |_: _|
			if param_name == "_" && ty_str.trim() == "_" {
				// No type annotation added
			} else {
				// Normal parameters or explicit type annotations
				output.push_str(": ");
				let cleaned = Self::clean_expression_spaces(&ty_str);
				output.push_str(&cleaned);
			}
		}
		output.push('|');
	}

	/// Format the page body.
	fn format_body(&self, output: &mut String, body: &PageBody, indent: usize, depth: usize) {
		for node in &body.nodes {
			self.format_node(output, node, indent, depth);
		}
	}

	/// Format a single node.
	///
	/// The `depth` parameter tracks recursion depth to prevent stack overflow
	/// from deeply nested templates. When the maximum depth is exceeded,
	/// the node is rendered as a raw token stream instead.
	fn format_node(&self, output: &mut String, node: &PageNode, indent: usize, depth: usize) {
		if depth > MAX_FORMAT_DEPTH {
			// Prevent stack overflow: emit a comment indicating depth limit
			let ind = self.make_indent(indent);
			output.push_str(&ind);
			output.push_str("/* formatting depth limit exceeded */\n");
			return;
		}

		match node {
			PageNode::Element(elem) => self.format_element(output, elem, indent, depth),
			PageNode::Text(text) => self.format_text(output, text, indent),
			PageNode::Expression(expr) => self.format_expression(output, expr, indent),
			PageNode::If(if_node) => self.format_if(output, if_node, indent, depth),
			PageNode::For(for_node) => self.format_for(output, for_node, indent, depth),
			PageNode::Component(comp) => self.format_component(output, comp, indent, depth),
			PageNode::Watch(watch_node) => self.format_watch(output, watch_node, indent, depth),
		}
	}

	/// Format an element node.
	fn format_element(&self, output: &mut String, elem: &PageElement, indent: usize, depth: usize) {
		let ind = self.make_indent(indent);

		// Check if element is empty (no attrs, events, or children)
		let is_empty = elem.attrs.is_empty() && elem.events.is_empty() && elem.children.is_empty();

		// Element tag
		output.push_str(&ind);
		output.push_str(&elem.tag.to_string());

		if is_empty {
			// Empty element: single line format
			output.push_str(" {}\n");
		} else {
			// Non-empty element: multi-line format
			output.push_str(" {\n");

			// Attributes (one per line)
			for attr in &elem.attrs {
				self.format_attr(output, attr, indent + 1);
			}

			// Event handlers (one per line)
			for event in &elem.events {
				self.format_event(output, event, indent + 1);
			}

			// Children
			for child in &elem.children {
				self.format_node(output, child, indent + 1, depth + 1);
			}

			// Closing brace
			output.push_str(&ind);
			output.push_str("}\n");
		}
	}

	/// Format an attribute.
	fn format_attr(&self, output: &mut String, attr: &PageAttr, indent: usize) {
		let ind = self.make_indent(indent);
		let value_str = Self::clean_expression_spaces(&attr.value.to_token_stream().to_string());
		output.push_str(&ind);
		output.push_str(&attr.name.to_string());
		output.push_str(": ");
		output.push_str(&value_str);
		output.push_str(",\n");
	}

	/// Format an event handler.
	///
	/// Uses rustfmt to format complex closures for better readability.
	/// Empty closures (e.g., `|_| {}`) are kept as-is.
	fn format_event(&self, output: &mut String, event: &PageEvent, indent: usize) {
		let ind = self.make_indent(indent);

		// Format handler with rustfmt (empty closures are kept as-is)
		let handler_str = self.format_handler_expression(&event.handler, indent + 1);

		output.push_str(&ind);
		output.push('@');
		output.push_str(&event.event_type.to_string());
		output.push_str(": ");
		output.push_str(&handler_str);
		output.push_str(",\n");
	}

	/// Format a text node.
	fn format_text(&self, output: &mut String, text: &PageText, indent: usize) {
		let ind = self.make_indent(indent);
		output.push_str(&ind);
		// Escape and quote the text
		let escaped = text.content.replace('\\', "\\\\").replace('"', "\\\"");
		output.push('"');
		output.push_str(&escaped);
		output.push_str("\"\n");
	}

	/// Clean up extra spaces in expression strings.
	fn clean_expression_spaces(s: &str) -> String {
		// Static regex compilation (compiled once, reused)
		// [\w:]+ matches identifiers and path-qualified names (e.g., Vec::new, std::iter::once)
		static IDENT_PAREN: LazyLock<Regex> = LazyLock::new(|| {
			Regex::new(r"([\w:]+) \(").expect("Failed to compile IDENT_PAREN regex")
		});
		static IDENT_MACRO: LazyLock<Regex> = LazyLock::new(|| {
			Regex::new(r"([\w:]+) !").expect("Failed to compile IDENT_MACRO regex")
		});
		// Match generic type opening: Result <T> -> Result<T>
		// Only matches when followed by an identifier (not =, <, > which indicate operators)
		static IDENT_ANGLE: LazyLock<Regex> = LazyLock::new(|| {
			Regex::new(r"([\w:>)]+) <([A-Za-z_&'\[(\*])")
				.expect("Failed to compile IDENT_ANGLE regex")
		});
		// Match generic type closing: String > -> String>
		// Only matches when preceded by an identifier/closing bracket and not followed by =, >, <
		static ANGLE_CLOSE: LazyLock<Regex> = LazyLock::new(|| {
			Regex::new(r"([\w>)]) >([\s,;)}\]>])").expect("Failed to compile ANGLE_CLOSE regex")
		});

		let s = s
			// Existing: Dot and method chaining
			.replace(" . ", ".")

			// Existing: Parentheses (function calls, tuples)
			.replace(" ( ", "(")
			.replace(" )", ")")
			.replace("( ", "(")
			.replace(" )", ")")
			.replace(" ()", "()")

			// Generic type angle brackets: Vec < String > -> Vec<String>
			// These handle spaces around < and > in generic type parameters
			// Note: We don't use ".replace("> ", ">")" because it would incorrectly
			// affect arrow operators like "-> Result" turning them into "->Result"
			.replace("< ", "<")
			.replace(" <", "<")
			.replace(" >", ">")

			// New: Path separator (std::vec::Vec)
			.replace(" :: ", "::")

			// New: Arrays and slices
			.replace("[ ", "[")
			.replace(" ]", "]")
			.replace(" ; ", "; ")  // Array size separator (preserve space after semicolon)

			// New: Reference types
			.replace("& ", "&")

			// New: Pointer types
			.replace("* const ", "*const ")
			.replace("* mut ", "*mut ")

			// New: Lifetime syntax
			.replace("for < ", "for<")
			.replace(" > fn", ">fn")

			// New: Comma in generics (Result<T, E>)
			.replace(" , ", ", ") // Note: Preserve space after comma

			// New: Macro calls (format! macro, etc.)
			.replace("! (", "!(") // Macro symbol before parenthesis
			.replace("! [", "![") // Macro with brackets
			.replace("! {", "!{") // Macro with braces

			// New: Closure parameter pipes
			// Handle closure syntax: | param | -> |param|
			// Note: OR operator (a | b) has different context and should be preserved
			.replace("| }", "|}") // Closing of empty closure before brace
			;

		// Apply regex replacements for identifier patterns
		let s = IDENT_PAREN.replace_all(&s, "$1("); // identifier ( -> identifier(
		let s = IDENT_MACRO.replace_all(&s, "$1!"); // identifier ! -> identifier!
		let s = IDENT_ANGLE.replace_all(&s, "$1<$2"); // identifier <T -> identifier<T (for generics)
		// Apply closing angle bracket repeatedly for nested generics like Option<String >
		let s = ANGLE_CLOSE.replace_all(&s, "$1>$2");

		// Handle closure pipes: | x | -> |x|, | x, y | -> |x, y|, || -> ||
		// This regex matches closure parameter lists between pipes
		static CLOSURE_PARAMS: LazyLock<Regex> = LazyLock::new(|| {
			Regex::new(r"\| ([^|]*?) \|").expect("Failed to compile CLOSURE_PARAMS regex")
		});
		let s = CLOSURE_PARAMS.replace_all(&s, |caps: &regex::Captures| {
			let inner = &caps[1];
			// Clean up spaces around commas in closure params
			let cleaned = inner.trim();
			format!("|{}|", cleaned)
		});

		s.into_owned()
	}

	/// Check if the expression is an empty closure (e.g., `|_| {}`, `|| {}`)
	///
	/// Empty closures are kept as-is without rustfmt formatting.
	fn is_empty_closure(expr: &syn::Expr) -> bool {
		if let syn::Expr::Closure(closure) = expr
			&& let syn::Expr::Block(block) = closure.body.as_ref()
		{
			return block.block.stmts.is_empty();
		}
		false
	}

	/// Format Rust code with rustfmt
	///
	/// Falls back to the input code if rustfmt is not available or fails.
	fn format_with_rustfmt(&self, code: &str) -> String {
		use std::io::Write;
		use std::process::Stdio;

		let mut cmd = Command::new("rustfmt");
		self.rustfmt_options.apply_to_command(&mut cmd);

		// Fallback to default edition if no config is specified
		if self.rustfmt_options.config_path.is_none() && self.rustfmt_options.edition.is_none() {
			cmd.arg("--edition=2024");
		}

		let child = cmd
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.spawn();

		match child {
			Ok(mut child_process) => {
				if let Some(stdin) = child_process.stdin.as_mut() {
					let _ = stdin.write_all(code.as_bytes());
				}
				match child_process.wait_with_output() {
					Ok(output) if output.status.success() => {
						String::from_utf8(output.stdout).unwrap_or_else(|_| code.to_string())
					}
					_ => code.to_string(),
				}
			}
			Err(_) => code.to_string(),
		}
	}

	/// Find the end of an expression considering nested braces
	fn find_expression_end(s: &str) -> Option<usize> {
		let mut brace_depth = 0;
		let mut paren_depth = 0;
		let mut in_string = false;
		let mut escape_next = false;

		for (i, c) in s.chars().enumerate() {
			if escape_next {
				escape_next = false;
				continue;
			}

			match c {
				'\\' if in_string => escape_next = true,
				'"' if !in_string => in_string = true,
				'"' if in_string => in_string = false,
				'{' if !in_string => brace_depth += 1,
				'}' if !in_string => brace_depth -= 1,
				'(' if !in_string => paren_depth += 1,
				')' if !in_string => paren_depth -= 1,
				';' if !in_string && brace_depth == 0 && paren_depth == 0 => return Some(i),
				_ => {}
			}
		}
		None
	}

	/// Extract the handler expression from the wrapper code
	///
	/// Pattern: `let _handler = <expr>;`
	fn extract_handler_from_wrapper(formatted: &str) -> Option<String> {
		let start_marker = "let _handler = ";
		let start = formatted.find(start_marker)? + start_marker.len();
		let handler_part = &formatted[start..];
		let end = Self::find_expression_end(handler_part)?;
		Some(handler_part[..end].trim().to_string())
	}

	/// Apply base indentation to each line of a multi-line handler
	fn apply_base_indent(&self, handler: &str, base_indent: usize) -> String {
		let lines: Vec<&str> = handler.lines().collect();

		if lines.len() == 1 {
			return handler.to_string();
		}

		// First line has no additional indent (format_event adds the base indent)
		// Subsequent lines get the base indent applied
		let indent_str = self.make_indent(base_indent);
		let mut result = lines[0].to_string();

		for line in &lines[1..] {
			result.push('\n');
			if !line.trim().is_empty() {
				result.push_str(&indent_str);
			}
			result.push_str(line);
		}

		result
	}

	/// Format an event handler expression with rustfmt
	///
	/// Empty closures are kept as-is, complex closures are formatted with rustfmt.
	fn format_handler_expression(&self, expr: &syn::Expr, base_indent: usize) -> String {
		// Empty closures are kept as-is
		if Self::is_empty_closure(expr) {
			return Self::clean_expression_spaces(&expr.to_token_stream().to_string());
		}

		// Wrap the expression in a valid Rust file
		let wrapper_code = format!(
			"fn _wrapper() {{ let _handler = {}; }}",
			expr.to_token_stream()
		);

		// Parse with syn
		let Ok(file) = syn::parse_file(&wrapper_code) else {
			return Self::clean_expression_spaces(&expr.to_token_stream().to_string());
		};

		// Format with prettyplease + rustfmt
		let prettyplease_output = prettyplease::unparse(&file);
		let formatted = self.format_with_rustfmt(&prettyplease_output);

		// Extract the formatted handler
		let Some(handler_str) = Self::extract_handler_from_wrapper(&formatted) else {
			return Self::clean_expression_spaces(&expr.to_token_stream().to_string());
		};

		// Apply base indentation
		self.apply_base_indent(&handler_str, base_indent)
	}

	/// Format an expression node.
	fn format_expression(&self, output: &mut String, expr: &PageExpression, indent: usize) {
		let ind = self.make_indent(indent);
		output.push_str(&ind);

		let expr_str = Self::clean_expression_spaces(&expr.expr.to_token_stream().to_string());

		if expr.braced {
			output.push_str("{ ");
			output.push_str(&expr_str);
			output.push_str(" }\n");
		} else {
			output.push_str(&expr_str);
			output.push('\n');
		}
	}

	/// Format an if node.
	fn format_if(&self, output: &mut String, if_node: &PageIf, indent: usize, depth: usize) {
		let ind = self.make_indent(indent);

		// if condition {
		output.push_str(&ind);
		output.push_str("if ");
		output.push_str(&Self::clean_expression_spaces(
			&if_node.condition.to_token_stream().to_string(),
		));
		output.push_str(" {\n");

		// then branch
		for node in &if_node.then_branch {
			self.format_node(output, node, indent + 1, depth + 1);
		}

		// else branch
		match &if_node.else_branch {
			Some(PageElse::Block(nodes)) => {
				output.push_str(&ind);
				output.push_str("} else {\n");
				for node in nodes {
					self.format_node(output, node, indent + 1, depth + 1);
				}
				output.push_str(&ind);
				output.push_str("}\n");
			}
			Some(PageElse::If(nested_if)) => {
				output.push_str(&ind);
				output.push_str("} else ");
				// Format the nested if without initial indent
				self.format_if_inline(output, nested_if, indent, depth + 1);
			}
			None => {
				output.push_str(&ind);
				output.push_str("}\n");
			}
		}
	}

	/// Format an if node inline (for else if chains).
	fn format_if_inline(&self, output: &mut String, if_node: &PageIf, indent: usize, depth: usize) {
		if depth > MAX_FORMAT_DEPTH {
			output.push_str("/* else-if chain depth limit exceeded */ {}\n");
			return;
		}

		let ind = self.make_indent(indent);

		output.push_str("if ");
		output.push_str(&Self::clean_expression_spaces(
			&if_node.condition.to_token_stream().to_string(),
		));
		output.push_str(" {\n");

		for node in &if_node.then_branch {
			self.format_node(output, node, indent + 1, depth + 1);
		}

		match &if_node.else_branch {
			Some(PageElse::Block(nodes)) => {
				output.push_str(&ind);
				output.push_str("} else {\n");
				for node in nodes {
					self.format_node(output, node, indent + 1, depth + 1);
				}
				output.push_str(&ind);
				output.push_str("}\n");
			}
			Some(PageElse::If(nested_if)) => {
				output.push_str(&ind);
				output.push_str("} else ");
				self.format_if_inline(output, nested_if, indent, depth + 1);
			}
			None => {
				output.push_str(&ind);
				output.push_str("}\n");
			}
		}
	}

	/// Format a for node.
	fn format_for(&self, output: &mut String, for_node: &PageFor, indent: usize, depth: usize) {
		let ind = self.make_indent(indent);

		output.push_str(&ind);
		output.push_str("for ");
		output.push_str(&Self::clean_expression_spaces(
			&for_node.pat.to_token_stream().to_string(),
		));
		output.push_str(" in ");
		output.push_str(&Self::clean_expression_spaces(
			&for_node.iter.to_token_stream().to_string(),
		));
		output.push_str(" {\n");

		for node in &for_node.body {
			self.format_node(output, node, indent + 1, depth + 1);
		}

		output.push_str(&ind);
		output.push_str("}\n");
	}

	/// Format a watch node.
	fn format_watch(
		&self,
		output: &mut String,
		watch_node: &reinhardt_pages::ast::PageWatch,
		indent: usize,
		depth: usize,
	) {
		let ind = self.make_indent(indent);

		output.push_str(&ind);
		output.push_str("watch {\n");

		self.format_node(output, &watch_node.expr, indent + 1, depth + 1);

		output.push_str(&ind);
		output.push_str("}\n");
	}

	/// Format a component call.
	fn format_component(
		&self,
		output: &mut String,
		comp: &PageComponent,
		indent: usize,
		depth: usize,
	) {
		let ind = self.make_indent(indent);

		output.push_str(&ind);
		output.push_str(&comp.name.to_string());
		output.push('(');

		// Arguments
		for (i, arg) in comp.args.iter().enumerate() {
			if i > 0 {
				output.push_str(", ");
			}
			output.push_str(&arg.name.to_string());
			output.push_str(": ");
			output.push_str(&Self::clean_expression_spaces(
				&arg.value.to_token_stream().to_string(),
			));
		}

		output.push(')');

		// Children
		if let Some(children) = &comp.children {
			output.push_str(" {\n");
			for child in children {
				self.format_node(output, child, indent + 1, depth + 1);
			}
			output.push_str(&ind);
			output.push('}');
		}

		output.push('\n');
	}

	/// Create indentation string.
	fn make_indent(&self, level: usize) -> String {
		self.indent.repeat(level)
	}

	/// Check if the file has an ignore-all marker at the beginning.
	///
	/// This checks the first 50 lines of the file for a comment containing
	/// `reinhardt-fmt:ignore-all`. The marker must appear before any code line.
	pub(crate) fn has_ignore_all_marker(&self, source: &str) -> bool {
		const MARKER: &str = "reinhardt-fmt:ignore-all";

		// Check only the first 50 lines for performance
		for line in source.lines().take(50) {
			let trimmed = line.trim();

			// Check comment lines only
			if let Some(comment) = trimmed.strip_prefix("//") {
				let comment_content = comment.trim();
				// Remove spaces for flexible matching
				if comment_content.replace(' ', "").contains(MARKER) {
					return true;
				}
			}

			// Stop at first code line (non-comment, non-empty)
			if !trimmed.is_empty() && !trimmed.starts_with("//") {
				break;
			}
		}
		false
	}

	/// Find all ignore ranges (off/on pairs) in the source code.
	///
	/// Returns a list of byte offset ranges where formatting should be skipped.
	/// Warns if there are nested 'off' markers or unmatched markers.
	fn find_ignore_ranges(&self, source: &str) -> Vec<(usize, usize)> {
		const OFF_MARKER: &str = "reinhardt-fmt:off";
		const ON_MARKER: &str = "reinhardt-fmt:on";

		let mut ranges = Vec::new();
		let mut current_off_start: Option<usize> = None;
		let mut byte_offset = 0;
		let total_len = source.len();

		for line in source.lines() {
			let trimmed = line.trim();

			if let Some(comment) = trimmed.strip_prefix("//") {
				let comment_content = comment.trim().replace(' ', "");

				if comment_content.contains(OFF_MARKER) {
					if current_off_start.is_some() {
						eprintln!(
							"Warning: Nested 'reinhardt-fmt: off' at byte {}",
							byte_offset
						);
						// Don't update current_off_start if already set (nested case)
					} else {
						current_off_start = Some(byte_offset);
					}
				} else if comment_content.contains(ON_MARKER) {
					if let Some(start) = current_off_start.take() {
						ranges.push((start, byte_offset));
					} else {
						eprintln!(
							"Warning: 'reinhardt-fmt: on' without matching 'off' at byte {}",
							byte_offset
						);
					}
				}
			}

			byte_offset += line.len() + 1; // +1 for newline
		}

		// Handle unclosed range - extend to end of file
		if let Some(start) = current_off_start {
			eprintln!("Warning: Unclosed 'reinhardt-fmt: off' at end of file");
			ranges.push((start, total_len));
		}

		ranges
	}

	/// Check if an individual macro has an ignore marker on the previous line.
	///
	/// The marker must be on the line immediately before the macro (no blank lines).
	fn has_individual_ignore_marker(&self, source: &str, macro_start: usize) -> bool {
		const MARKER: &str = "reinhardt-fmt:ignore";

		// If macro is at the start of the file, no previous line exists
		if macro_start == 0 {
			return false;
		}

		// Find the start of the current line (where the macro is)
		let line_start = source[..macro_start]
			.rfind('\n')
			.map(|pos| pos + 1)
			.unwrap_or(0);

		// If this is the first line, no previous line exists
		if line_start == 0 {
			return false;
		}

		// Find the end of the previous line (newline character position)
		let prev_line_end = line_start - 1; // This is the '\n' character

		// Find the start of the previous line
		let prev_line_start = source[..prev_line_end]
			.rfind('\n')
			.map(|pos| pos + 1)
			.unwrap_or(0);

		// Extract the previous line
		let prev_line = &source[prev_line_start..prev_line_end];
		let trimmed = prev_line.trim();

		// Check if it's a comment with the ignore marker
		if let Some(comment) = trimmed.strip_prefix("//") {
			let comment_content = comment.trim().replace(' ', "");
			return comment_content.contains(MARKER);
		}

		false
	}

	/// Apply ignore markers to macros, setting their should_skip flags.
	///
	/// Priority order:
	/// 1. Individual macro ignore (highest) - implemented in Phase 3
	/// 2. Range ignore (medium) - implemented in Phase 2
	/// 3. File-wide ignore (lowest) - handled in format() method
	fn apply_ignore_markers(&self, source: &str, macros: &mut [MacroInfo]) {
		// Find all ignore ranges
		let ignore_ranges = self.find_ignore_ranges(source);

		// Apply markers to each macro
		for macro_info in macros.iter_mut() {
			// Priority 1: Individual macro ignore (highest priority)
			if self.has_individual_ignore_marker(source, macro_info.start) {
				macro_info.should_skip = true;
				continue;
			}

			// Priority 2: Range ignore (medium priority)
			for (range_start, range_end) in &ignore_ranges {
				if macro_info.start >= *range_start && macro_info.start < *range_end {
					macro_info.should_skip = true;
					break;
				}
			}

			// Priority 3: File-wide ignore is already handled in format()
		}
	}

	/// Protect page! macros by replacing them with placeholders.
	///
	/// This allows rustfmt to process the surrounding Rust code without
	/// modifying the page! macro contents. The macros can be restored
	/// using `restore_page_macros`.
	///
	/// # Placeholder Format
	///
	/// Each page! macro is replaced with:
	/// ```text
	/// __reinhardt_placeholder__!(/*n*/)
	/// ```
	/// where `n` is a unique identifier.
	///
	/// # Example
	///
	/// ```text
	/// // Before:
	/// let view = page!(|| { div { "hello" } })(props);
	///
	/// // After:
	/// let view = __reinhardt_placeholder__!(/*0*/)(props);
	/// ```
	pub(crate) fn protect_page_macros(&self, content: &str) -> ProtectResult {
		// Quick check: if no page! pattern exists, return unchanged
		if !content.contains("page!(") {
			return ProtectResult {
				protected_content: content.to_string(),
				backups: Vec::new(),
			};
		}

		// Find all page! macros
		let macros = match self.find_page_macros(content) {
			Ok(m) => m,
			Err(_) => {
				return ProtectResult {
					protected_content: content.to_string(),
					backups: Vec::new(),
				};
			}
		};

		if macros.is_empty() {
			return ProtectResult {
				protected_content: content.to_string(),
				backups: Vec::new(),
			};
		}

		// Sort macros by position
		let mut macros = macros;
		macros.sort_by_key(|m| m.start);

		// Build result by replacing each macro with placeholder
		let mut result = String::with_capacity(content.len());
		let mut backups = Vec::with_capacity(macros.len());
		let mut last_end = 0;

		for (id, macro_info) in macros.iter().enumerate() {
			// Copy content before this macro
			result.push_str(&content[last_end..macro_info.start]);

			// Save original macro text
			let original = content[macro_info.start..macro_info.end].to_string();
			backups.push(PageMacroBackup { id, original });

			// Insert placeholder (macro format so rustfmt doesn't touch it)
			result.push_str(&format!("__reinhardt_placeholder__!(/*{}*/)", id));

			last_end = macro_info.end;
		}

		// Copy remaining content
		result.push_str(&content[last_end..]);

		ProtectResult {
			protected_content: result,
			backups,
		}
	}

	/// Restore page! macros from placeholders.
	///
	/// This reverses the effect of `protect_page_macros`, replacing
	/// placeholders with the original page! macro content.
	pub(crate) fn restore_page_macros(content: &str, backups: &[PageMacroBackup]) -> String {
		if backups.is_empty() {
			return content.to_string();
		}

		let mut result = content.to_string();

		// Replace placeholders in reverse order to maintain correct positions
		for backup in backups.iter().rev() {
			let placeholder = format!("__reinhardt_placeholder__!(/*{}*/)", backup.id);
			result = result.replace(&placeholder, &backup.original);
		}

		result
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_format_simple_element() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|| { div { "hello" } })"#;
		let result = formatter.format(input).unwrap();

		assert!(result.content.contains("div {"));
		assert!(result.content.contains("\"hello\""));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_format_with_attributes() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|| { div class="foo" { "hello" } })"#;
		let result = formatter.format(input).unwrap();

		assert!(result.content.contains("div"));
		assert!(result.content.contains("class"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_no_change_non_page() {
		let formatter = AstPageFormatter::new();
		let input = "fn main() { println!(\"hello\"); }";
		let result = formatter.format(input).unwrap();

		assert_eq!(input, result.content);
		assert!(!result.contains_page_macro);
	}

	#[test]
	fn test_skip_page_in_string() {
		let formatter = AstPageFormatter::new();
		let input = r#"fn main() { let s = "page!(|| { div { } })"; }"#;
		let result = formatter.format(input).unwrap();

		assert!(result.content.contains("page!(|| { div { } })"));
		assert!(!result.contains_page_macro);
	}

	#[test]
	fn test_skip_page_in_comment() {
		let formatter = AstPageFormatter::new();
		let input = r#"// page!(|| { div { } })
fn main() {}"#;
		let result = formatter.format(input).unwrap();

		assert!(result.content.contains("// page!(|| { div { } })"));
		assert!(!result.contains_page_macro);
	}

	#[test]
	fn test_format_with_params() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|name: String| { div { { name } } })"#;
		let result = formatter.format(input).unwrap();

		assert!(result.content.contains("name: String"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_format_nested_elements() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|| { div { p { "hello" } } })"#;
		let result = formatter.format(input).unwrap();

		assert!(result.content.contains("div {"));
		assert!(result.content.contains("p {"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_format_if_node() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|| { @if true { div { } } })"#;
		let result = formatter.format(input).unwrap();

		assert!(result.content.contains("@if"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_format_for_node() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|| { @for item in items { div { } } })"#;
		let result = formatter.format(input).unwrap();

		assert!(result.content.contains("@for"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_format_component() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|| { <MyComponent /> })"#;
		let result = formatter.format(input).unwrap();

		assert!(result.content.contains("<MyComponent"));
		assert!(result.content.contains("/>"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_format_event_handler() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|| { button { @click: |_| {}, "Click" } })"#;
		let result = formatter.format(input).unwrap();

		assert!(result.content.contains("@click"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_safety_complex_non_page_file() {
		let formatter = AstPageFormatter::new();
		let input = r#"
//! Module documentation

use std::collections::HashMap;

/// A complex struct
#[derive(Debug, Clone)]
pub struct MyStruct<T> {
	field: T,
}

impl<T> MyStruct<T> {
	pub fn new(field: T) -> Self {
		Self { field }
	}
}

// Some comment about the function
fn complex_function(x: i32, y: i32) -> i32 {
	x + y
}

#[cfg(test)]
mod tests {
	#[test]
	fn test_something() {
		assert_eq!(2 + 2, 4);
	}
}
"#;
		let result = formatter.format(input).unwrap();
		assert_eq!(input, result.content);
		assert!(!result.contains_page_macro);
	}

	// ========================================
	// Tests for generic type formatting
	// ========================================

	#[test]
	fn test_format_params_with_vec() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|items: Vec<String>| { div { } })"#;
		let result = formatter.format(input).unwrap();
		assert!(result.content.contains("items: Vec<String>"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_format_params_with_option() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|value: Option<i32>| { div { } })"#;
		let result = formatter.format(input).unwrap();
		assert!(result.content.contains("value: Option<i32>"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_format_params_with_result() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|res: Result<String, Error>| { div { } })"#;
		let result = formatter.format(input).unwrap();
		assert!(result.content.contains("res: Result<String, Error>"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_format_params_with_nested_generics() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|items: Vec<Option<String>>| { div { } })"#;
		let result = formatter.format(input).unwrap();
		assert!(result.content.contains("items: Vec<Option<String>>"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_format_params_with_multiple_generics() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|map: HashMap<String, i32>| { div { } })"#;
		let result = formatter.format(input).unwrap();
		assert!(result.content.contains("map: HashMap<String, i32>"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_format_params_with_references() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|s: &str| { div { } })"#;
		let result = formatter.format(input).unwrap();
		assert!(result.content.contains("s: &str"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_format_params_with_arrays() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|arr: [i32; 5]| { div { } })"#;
		let result = formatter.format(input).unwrap();
		assert!(result.content.contains("arr: [i32; 5]"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_format_params_with_tuples() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|t: (String, i32)| { div { } })"#;
		let result = formatter.format(input).unwrap();
		assert!(result.content.contains("t: (String, i32)"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_format_params_with_path_types() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|v: std::vec::Vec<String>| { div { } })"#;
		let result = formatter.format(input).unwrap();
		assert!(result.content.contains("v: std::vec::Vec<String>"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_format_params_with_complex_types() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|f: Box<dyn Fn() -> Result<(), Error>>| { div { } })"#;
		let result = formatter.format(input).unwrap();
		assert!(
			result
				.content
				.contains("f: Box<dyn Fn() -> Result<(), Error>>")
		);
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_format_params_types_idempotent() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|vec: Vec<String>, opt: Option<i32>, res: Result<String, Error>| { div { } })"#;
		let result = formatter.format(input).unwrap();

		// Format again to ensure idempotency
		let result2 = formatter.format(&result.content).unwrap();
		assert_eq!(result.content, result2.content);
		assert!(result2.contains_page_macro);
	}

	#[test]
	fn test_format_macro_calls() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|| {
			div { { format!("Hello {}", name) } }
		})"#;
		let result = formatter.format(input).unwrap();

		assert!(result.content.contains("format!"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_format_function_calls() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|| {
			div { { get_message() } }
		})"#;
		let result = formatter.format(input).unwrap();

		assert!(result.content.contains("get_message()"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_format_method_calls() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|| {
			div { { user.get_name() } }
		})"#;
		let result = formatter.format(input).unwrap();

		assert!(result.content.contains("user.get_name()"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_format_complex_event_handler() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|| {
			button {
				@click: |event| {
					prevent_default(event);
					handle_click();
				},
				"Click Me"
			}
		})"#;

		let result = formatter.format(input).unwrap();

		// Verify structure is preserved
		assert!(result.content.contains("button"));
		assert!(result.content.contains("@click"));
		assert!(result.content.contains("|event|"));
		assert!(result.content.contains("prevent_default(event)"));
		assert!(result.content.contains("handle_click()"));
		assert!(result.content.contains("\"Click Me\""));

		// Format should be idempotent
		let result2 = formatter.format(&result.content).unwrap();
		assert_eq!(result.content, result2.content);
		assert!(result2.contains_page_macro);
	}

	#[test]
	fn test_format_function_macro_calls_idempotent() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|| {
			div {
				{ format!("Count: {}", count) }
				{ get_user().name() }
				{ vec![1, 2, 3].len() }
			}
		})"#;

		let result = formatter.format(input).unwrap();
		let result2 = formatter.format(&result.content).unwrap();
		assert_eq!(result.content, result2.content);
		assert!(result2.contains_page_macro);
	}

	// ==================== Ignore Marker Tests ====================

	#[test]
	fn test_ignore_all_at_file_start() {
		let formatter = AstPageFormatter::new();
		let input = r#"// reinhardt-fmt: ignore-all

page!(|| {
div{badly}
})"#;

		let result = formatter.format(input).unwrap();

		// Should keep original formatting
		assert_eq!(input, result.content);
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_ignore_all_after_module_doc() {
		let formatter = AstPageFormatter::new();
		let input = r#"//! Module documentation
// reinhardt-fmt: ignore-all

page!(|| {
div{badly}
})"#;

		let result = formatter.format(input).unwrap();

		// Should keep original formatting
		assert_eq!(input, result.content);
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_ignore_all_not_at_start() {
		// When ignore-all marker appears AFTER code lines, it should NOT be recognized
		// because the marker must appear BEFORE any code line (as documented).
		let formatter = AstPageFormatter::new();
		let input = r#"use foo;

// reinhardt-fmt: ignore-all

page!(|| {
div{badly}
})"#;

		let result = formatter.format(input).unwrap();

		// Marker after code line is NOT recognized, so formatting IS applied
		assert!(result.contains_page_macro);
		// The page! macro content should be formatted (indentation added)
		// Original: "div{badly}" -> Formatted: "div {\n\t\tbadly\n\t}"
		assert!(result.content.contains("div {"));
		assert!(result.content.contains("badly"));
	}

	#[test]
	fn test_ignore_range_basic() {
		let formatter = AstPageFormatter::new();
		let input = r#"// reinhardt-fmt: ignore-on
page!(|| {
div{badly}
})
// reinhardt-fmt: ignore-off

page!(|| { div { "formatted" } })"#;

		let result = formatter.format(input).unwrap();

		// First macro should be unchanged
		assert!(result.content.contains("div{badly}"));
		// Second macro should be formatted
		assert!(result.content.contains("div {"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_ignore_range_nested_warning() {
		let formatter = AstPageFormatter::new();
		let input = r#"// reinhardt-fmt: ignore-on
page!(|| { div { "first" } })
// reinhardt-fmt: ignore-on
page!(|| { div { "second" } })
// reinhardt-fmt: ignore-off
page!(|| { div { "third" } })"#;

		let result = formatter.format(input).unwrap();

		// All should be kept as-is
		assert!(result.content.contains("first"));
		assert!(result.content.contains("second"));
		assert!(result.content.contains("third"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_ignore_range_unmatched_on() {
		let formatter = AstPageFormatter::new();
		let input = r#"// reinhardt-fmt: ignore-on
page!(|| { div { "first" } })
page!(|| { div { "second" } })"#;

		let result = formatter.format(input).unwrap();

		// Both should be unchanged (ignore-on without ignore-off)
		assert!(result.content.contains("first"));
		assert!(result.content.contains("second"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_ignore_range_unclosed() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|| { div { "before" } })
// reinhardt-fmt: ignore-on
page!(|| { div{badly} })"#;

		let result = formatter.format(input).unwrap();

		// Second macro should be unchanged
		assert!(result.content.contains("div{badly}"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_multiple_ignore_ranges() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|| { div { "formatted1" } })
// reinhardt-fmt: ignore-on
page!(|| { div{ignored1} })
// reinhardt-fmt: ignore-off
page!(|| { div { "formatted2" } })
// reinhardt-fmt: ignore-on
page!(|| { div{ignored2} })
// reinhardt-fmt: ignore-off
page!(|| { div { "formatted3" } })"#;

		let result = formatter.format(input).unwrap();

		// Ignored macros should keep bad formatting
		assert!(result.content.contains("div{ignored1}"));
		assert!(result.content.contains("div{ignored2}"));
		// Formatted macros should have good formatting
		assert!(result.content.contains("div {"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_individual_ignore_basic() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|| { div { "formatted" } })

// reinhardt-fmt: ignore
page!(|| { div{ignored} })

page!(|| { div { "formatted" } })"#;

		let result = formatter.format(input).unwrap();

		// Middle macro should be unchanged
		assert!(result.content.contains("div{ignored}"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_individual_ignore_with_blank_line() {
		// When there's a blank line between the ignore marker and the macro,
		// the marker should NOT be recognized (as documented: marker must be on
		// the line immediately before the macro, with no blank lines).
		let formatter = AstPageFormatter::new();
		let input = r#"// reinhardt-fmt: ignore

page!(|| { div{ignored} })"#;

		let result = formatter.format(input).unwrap();

		// Marker is NOT recognized due to blank line, so formatting IS applied
		assert!(result.contains_page_macro);
		// The page! macro content should be formatted (indentation and spacing added)
		// Original: "div{ignored}" -> Formatted: "div {\n\t\tignored\n\t}"
		assert!(result.content.contains("div {"));
		assert!(result.content.contains("ignored"));
	}

	#[test]
	fn test_individual_ignore_multiple() {
		let formatter = AstPageFormatter::new();
		let input = r#"// reinhardt-fmt: ignore
page!(|| { div{ignored1} })

page!(|| { div { "formatted" } })

// reinhardt-fmt: ignore
page!(|| { div{ignored2} })"#;

		let result = formatter.format(input).unwrap();

		// Both ignored macros should be unchanged
		assert!(result.content.contains("div{ignored1}"));
		assert!(result.content.contains("div{ignored2}"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_individual_ignore_mixed_with_format() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|| { div { "formatted1" } })

// reinhardt-fmt: ignore
page!(|| { div{ignored} })

page!(|| { div { "formatted2" } })"#;

		let result = formatter.format(input).unwrap();

		// Ignored macro should keep bad formatting
		assert!(result.content.contains("div{ignored}"));
		// Other macros should be formatted
		assert!(result.content.contains("div {"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_individual_ignore_with_range() {
		let formatter = AstPageFormatter::new();
		let input = r#"// reinhardt-fmt: ignore-on
page!(|| { div{range_ignored} })
// reinhardt-fmt: ignore-off

// reinhardt-fmt: ignore
page!(|| { div{individual_ignored} })

page!(|| { div { "formatted" } })"#;

		let result = formatter.format(input).unwrap();

		// Both ignored macros should be unchanged
		assert!(result.content.contains("div{range_ignored}"));
		assert!(result.content.contains("div{individual_ignored}"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_individual_ignore_priority() {
		let formatter = AstPageFormatter::new();
		let input = r#"// reinhardt-fmt: ignore-on
// reinhardt-fmt: ignore
page!(|| { div{ignored} })
// reinhardt-fmt: ignore-off"#;

		let result = formatter.format(input).unwrap();

		// Should be unchanged (both markers apply)
		assert!(result.content.contains("div{ignored}"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_individual_ignore_at_file_start() {
		let formatter = AstPageFormatter::new();
		let input = r#"// reinhardt-fmt: ignore
page!(|| { div{ignored} })"#;

		let result = formatter.format(input).unwrap();

		// Should be unchanged
		assert!(result.content.contains("div{ignored}"));
		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_contains_page_macro_field_with_macro() {
		let formatter = AstPageFormatter::new();
		let input = r#"page!(|| { div { } })"#;
		let result = formatter.format(input).unwrap();

		assert!(result.contains_page_macro);
	}

	#[test]
	fn test_contains_page_macro_field_without_macro() {
		let formatter = AstPageFormatter::new();
		let input = r#"fn main() { println!("test"); }"#;
		let result = formatter.format(input).unwrap();

		assert!(!result.contains_page_macro);
	}

	#[test]
	fn test_ignore_all_with_page_macro() {
		let formatter = AstPageFormatter::new();
		let input = r#"// reinhardt-fmt: ignore-all
page!(|| { div { bad } })"#;
		let result = formatter.format(input).unwrap();

		assert_eq!(result.content, input); // Content unchanged
		assert!(result.contains_page_macro); // But macro is present
	}

	#[test]
	fn test_ignore_all_without_page_macro() {
		let formatter = AstPageFormatter::new();
		let input = r#"// reinhardt-fmt: ignore-all
fn main() {}"#;
		let result = formatter.format(input).unwrap();

		assert_eq!(result.content, input);
		assert!(!result.contains_page_macro); // No macro
	}

	// ==================== Protect/Restore Tests ====================

	#[test]
	fn test_protect_no_page_macro() {
		let formatter = AstPageFormatter::new();
		let input = "fn main() { println!(\"hello\"); }";
		let result = formatter.protect_page_macros(input);

		assert_eq!(result.protected_content, input);
		assert!(result.backups.is_empty());
	}

	#[test]
	fn test_protect_single_macro() {
		let formatter = AstPageFormatter::new();
		let input = r#"let view = page!(|| { div { "hello" } });"#;
		let result = formatter.protect_page_macros(input);

		assert!(
			result
				.protected_content
				.contains("__reinhardt_placeholder__!(/*0*/)")
		);
		assert!(!result.protected_content.contains("page!("));
		assert_eq!(result.backups.len(), 1);
		assert_eq!(result.backups[0].id, 0);
		assert!(result.backups[0].original.starts_with("page!("));
	}

	#[test]
	fn test_protect_multiple_macros() {
		let formatter = AstPageFormatter::new();
		let input = r#"
let view1 = page!(|| { div { "first" } });
let view2 = page!(|| { div { "second" } });
"#;
		let result = formatter.protect_page_macros(input);

		assert!(
			result
				.protected_content
				.contains("__reinhardt_placeholder__!(/*0*/)")
		);
		assert!(
			result
				.protected_content
				.contains("__reinhardt_placeholder__!(/*1*/)")
		);
		assert!(!result.protected_content.contains("page!("));
		assert_eq!(result.backups.len(), 2);
	}

	#[test]
	fn test_protect_preserves_surrounding_code() {
		let formatter = AstPageFormatter::new();
		let input = r#"use foo::bar;

fn render() -> View {
    page!(|| { div { "hello" } })
}

fn main() {}"#;
		let result = formatter.protect_page_macros(input);

		assert!(result.protected_content.contains("use foo::bar;"));
		assert!(result.protected_content.contains("fn render() -> View"));
		assert!(result.protected_content.contains("fn main() {}"));
		assert!(
			result
				.protected_content
				.contains("__reinhardt_placeholder__!(/*0*/)")
		);
	}

	#[test]
	fn test_restore_single_macro() {
		let formatter = AstPageFormatter::new();
		let original = r#"let view = page!(|| { div { "hello" } });"#;

		// Protect
		let protected = formatter.protect_page_macros(original);

		// Restore
		let restored =
			AstPageFormatter::restore_page_macros(&protected.protected_content, &protected.backups);

		assert_eq!(restored, original);
	}

	#[test]
	fn test_restore_multiple_macros() {
		let formatter = AstPageFormatter::new();
		let original = r#"
let view1 = page!(|| { div { "first" } });
let view2 = page!(|| { div { "second" } });
"#;

		// Protect
		let protected = formatter.protect_page_macros(original);

		// Restore
		let restored =
			AstPageFormatter::restore_page_macros(&protected.protected_content, &protected.backups);

		assert_eq!(restored, original);
	}

	#[test]
	fn test_protect_restore_roundtrip_complex() {
		let formatter = AstPageFormatter::new();
		let original = r#"use reinhardt::pages::page;

fn header() -> View {
    page!(|| {
        div {
            class: "header",
            h1 { "Title" }
        }
    })
}

fn footer() -> View {
    page!(|year: i32| {
        div {
            class: "footer",
            { format!("Copyright {}", year) }
        }
    })
}

fn main() {
    let _h = header();
    let _f = footer();
}"#;

		// Protect
		let protected = formatter.protect_page_macros(original);
		assert_eq!(protected.backups.len(), 2);
		assert!(!protected.protected_content.contains("page!("));

		// Restore
		let restored =
			AstPageFormatter::restore_page_macros(&protected.protected_content, &protected.backups);
		assert_eq!(restored, original);
	}

	#[test]
	fn test_protect_empty_backups_restore() {
		let content = "fn main() {}";
		let backups: Vec<PageMacroBackup> = Vec::new();

		let restored = AstPageFormatter::restore_page_macros(content, &backups);
		assert_eq!(restored, content);
	}

	#[test]
	fn test_protect_with_trailing_call() {
		let formatter = AstPageFormatter::new();
		let input = r#"let view = page!(|props: Props| { div { } })(props);"#;
		let result = formatter.protect_page_macros(input);

		// The trailing "(props)" should remain after the placeholder
		assert!(
			result
				.protected_content
				.contains("__reinhardt_placeholder__!(/*0*/)(props)")
		);
		assert_eq!(result.backups.len(), 1);

		// Restore should bring back original
		let restored =
			AstPageFormatter::restore_page_macros(&result.protected_content, &result.backups);
		assert_eq!(restored, input);
	}

	// ==================== Unicode Character Tests ====================

	#[test]
	fn test_find_matching_paren_with_emoji() {
		let source = r#"(div { "😀" })"#;
		let result = find_matching_paren(source, 1);
		assert_eq!(result, Some(source.len() - 1));
	}

	#[test]
	fn test_find_matching_paren_with_cjk() {
		let source = r#"(div { "日本語" })"#;
		let result = find_matching_paren(source, 1);
		assert_eq!(result, Some(source.len() - 1));
	}

	#[test]
	fn test_find_matching_paren_nested_with_unicode() {
		let source = r#"(outer { (inner { "안녕" }) })"#;
		let result = find_matching_paren(source, 1);
		assert_eq!(result, Some(source.len() - 1));
	}

	#[test]
	fn test_find_matching_paren_mixed() {
		let source = r#"(div { "Hello 世界 مرحبا" })"#;
		let result = find_matching_paren(source, 1);
		assert_eq!(result, Some(source.len() - 1));
	}

	#[test]
	fn test_is_in_comment_or_string_unicode_in_string() {
		let formatter = AstPageFormatter::new();
		let content = r#"let s = "😀🎉日本語";"#;
		let pos_in_string = content.find("日").unwrap();
		assert!(formatter.is_in_comment_or_string(content, pos_in_string));
	}

	#[test]
	fn test_is_in_comment_or_string_unicode_in_comment() {
		let formatter = AstPageFormatter::new();
		let content = r#"// This is a comment with 日本語"#;
		let pos_in_comment = content.find("日").unwrap();
		assert!(formatter.is_in_comment_or_string(content, pos_in_comment));
	}

	#[test]
	fn test_protect_restore_with_unicode_content() {
		let formatter = AstPageFormatter::new();
		let original = r#"let view = page!(|| { div { "😀🎉日本語" } });"#;

		let protected = formatter.protect_page_macros(original);
		assert_eq!(protected.backups.len(), 1);
		assert!(
			protected
				.protected_content
				.contains("__reinhardt_placeholder__")
		);

		let restored =
			AstPageFormatter::restore_page_macros(&protected.protected_content, &protected.backups);
		assert_eq!(restored, original);
	}
}
