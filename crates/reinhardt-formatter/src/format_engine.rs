//! Tree-sitter + Topiary based formatter engine for Reinhardt DSL macros.
//!
//! Rust source files are parsed with tree-sitter-rust so `page!`, `form!`,
//! `head!`, and `style!` invocations are detected as syntax nodes rather than
//! by text search.
//! Each DSL body is then parsed by a small Reinhardt tree-sitter grammar and
//! formatted through Topiary query captures. Supported `page!` Rust expression
//! islands are then passed through rustfmt conservatively; invalid or
//! unsupported islands are preserved unchanged. Surrounding Rust source
//! formatting remains the responsibility of the outer rustfmt pass.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use topiary_core::{
	Language as TopiaryLanguage, Operation, TopiaryQuery, formatter_str as topiary_formatter_str,
};
use topiary_tree_sitter_facade::Language as TopiaryTreeSitterLanguage;
use tree_sitter::{Language, Node, Parser};

/// Reason why formatting was skipped for a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SkipReason {
	/// File-wide ignore-all marker detected.
	FileWideMarker,
	/// All DSL macros were individually ignored.
	AllMacrosIgnored,
}

impl std::fmt::Display for SkipReason {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::FileWideMarker => write!(f, "file-wide ignore marker"),
			Self::AllMacrosIgnored => write!(f, "all macros ignored"),
		}
	}
}

/// Options to pass to rustfmt.
#[derive(Clone, Debug, Default)]
pub(crate) struct RustfmtOptions {
	/// Path to rustfmt.toml configuration file.
	pub config_path: Option<PathBuf>,
	/// Rust edition to use.
	pub edition: Option<String>,
	/// Rustfmt style edition to use.
	pub style_edition: Option<String>,
	/// Inline rustfmt config options.
	pub config: Option<String>,
	/// Rustfmt color setting.
	pub color: Option<String>,
}

impl RustfmtOptions {
	/// Apply these options to a rustfmt command.
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

	fn apply_to_island_command(&self, cmd: &mut Command) {
		self.apply_to_command(cmd);
		if self.edition.is_none() {
			cmd.arg("--edition=2024");
		}
	}
}

/// Result of formatting operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FormatResult {
	/// Formatted content.
	pub content: String,
	/// Whether the file contains a DSL macro.
	pub contains_dsl_macro: bool,
	/// If set, formatting was skipped for this reason.
	pub skipped: Option<SkipReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MacroKind {
	Page,
	Form,
	Head,
	Style,
}

impl MacroKind {
	fn name(self) -> &'static str {
		match self {
			Self::Page => "page",
			Self::Form => "form",
			Self::Head => "head",
			Self::Style => "style",
		}
	}

	fn query(self) -> &'static str {
		match self {
			Self::Page => include_str!("../queries/page_formatting.scm"),
			Self::Form => include_str!("../queries/form_formatting.scm"),
			Self::Head => include_str!("../queries/head_formatting.scm"),
			Self::Style => include_str!("../queries/style_formatting.scm"),
		}
	}

	fn grammar(self) -> Language {
		match self {
			Self::Page => tree_sitter_reinhardt_page::LANGUAGE.into(),
			Self::Form => tree_sitter_reinhardt_form::LANGUAGE.into(),
			Self::Head => tree_sitter_reinhardt_head::LANGUAGE.into(),
			Self::Style => tree_sitter_reinhardt_style::LANGUAGE.into(),
		}
	}
}

#[derive(Debug, Clone)]
struct MacroInfo {
	start: usize,
	end: usize,
	kind: MacroKind,
	should_skip: bool,
}

#[derive(Debug)]
struct MacroParts<'a> {
	open: char,
	close: char,
	inner: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProtectedRustfmtIsland {
	marker: String,
	original: String,
}

#[derive(Debug)]
struct FormattedDsl {
	output: String,
	protected_islands: Vec<ProtectedRustfmtIsland>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProtectedStyleBody {
	marker: String,
	original_body: String,
	original_indent: usize,
}

/// Style bodies temporarily replaced while rustfmt owns the surrounding Rust.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProtectedStyleBodies {
	bodies: Vec<ProtectedStyleBody>,
}

impl ProtectedStyleBodies {
	pub(crate) fn is_empty(&self) -> bool {
		self.bodies.is_empty()
	}
}

/// Replace every bare `style!` body with a unique Rust identifier marker.
pub(crate) fn protect_style_bodies(
	content: &str,
) -> Result<(String, ProtectedStyleBodies), String> {
	let macros = find_dsl_macros(content)?;
	let mut protected = content.to_string();
	let mut bodies = Vec::new();
	let mut marker_index = 0usize;

	for info in macros
		.into_iter()
		.filter(|info| info.kind == MacroKind::Style)
		.rev()
	{
		let original = &content[info.start..info.end];
		let parts = split_macro(original, MacroKind::Style)?;
		let inner_start = parts.inner.as_ptr() as usize - original.as_ptr() as usize;
		let inner_end = inner_start + parts.inner.len();
		let marker = loop {
			let candidate = format!("__reinhardt_style_body_{marker_index}");
			marker_index += 1;
			if !content.contains(&candidate) {
				break candidate;
			}
		};
		let original_indent = base_indent(content, info.start);
		protected.replace_range(info.start + inner_start..info.start + inner_end, &marker);
		bodies.push(ProtectedStyleBody {
			marker,
			original_body: parts.inner.to_string(),
			original_indent,
		});
	}
	bodies.reverse();

	Ok((protected, ProtectedStyleBodies { bodies }))
}

/// Restore every protected style body after rustfmt and follow its new indentation.
pub(crate) fn restore_style_bodies(
	content: &str,
	protected: &ProtectedStyleBodies,
) -> Result<String, String> {
	let mut restored = content.to_string();
	for body in &protected.bodies {
		let mut matches = restored.match_indices(&body.marker);
		let Some((marker_offset, _)) = matches.next() else {
			return Err(format!("rustfmt removed style body marker {}", body.marker));
		};
		if matches.next().is_some() {
			return Err(format!(
				"rustfmt duplicated style body marker {}",
				body.marker
			));
		}
		let indentation = base_indent(&restored, marker_offset);
		let replacement =
			reindent_style_body(&body.original_body, body.original_indent, indentation);
		restored.replace_range(
			marker_offset..marker_offset + body.marker.len(),
			&replacement,
		);
	}
	Ok(restored)
}

fn reindent_style_body(body: &str, from: usize, to: usize) -> String {
	let from_prefix = "\t".repeat(from);
	let to_prefix = "\t".repeat(to);
	body.split('\n')
		.enumerate()
		.map(|(index, line)| {
			if index == 0 {
				return line.to_string();
			}
			let relative = line.strip_prefix(&from_prefix).unwrap_or(line);
			if line.is_empty() {
				String::new()
			} else {
				format!("{to_prefix}{relative}")
			}
		})
		.collect::<Vec<_>>()
		.join("\n")
}

/// Formatter engine used by `fmt` and `fmt-all`.
#[derive(Debug, Default)]
pub(crate) struct FormatEngine {
	rustfmt_options: RustfmtOptions,
}

impl FormatEngine {
	/// Create a new formatter engine.
	#[cfg(test)]
	pub(crate) fn new() -> Self {
		Self::default()
	}

	/// Create a formatter engine with rustfmt options for page! Rust islands.
	pub(crate) fn with_rustfmt_options(rustfmt_options: RustfmtOptions) -> Self {
		Self { rustfmt_options }
	}

	/// Check if a source file has a file-wide ignore marker.
	pub(crate) fn has_ignore_all_marker(&self, content: &str) -> bool {
		content
			.lines()
			.take(50)
			.take_while(|line| {
				let trimmed = line.trim();
				trimmed.is_empty()
					|| trimmed.starts_with("//")
					|| trimmed.starts_with("/*")
					|| trimmed.starts_with("#![")
			})
			.any(|line| {
				marker_matches(line, "reinhardt-fmt:ignore-all") || rustfmt_skip_attr_matches(line)
			})
	}

	/// Format all supported DSL macros in a Rust source string.
	pub(crate) fn format(&self, content: &str) -> Result<FormatResult, String> {
		let mut macros = find_dsl_macros(content)?;
		if macros.is_empty() {
			return Ok(FormatResult {
				content: content.to_string(),
				contains_dsl_macro: false,
				skipped: None,
			});
		}

		if self.has_ignore_all_marker(content) {
			return Ok(FormatResult {
				content: content.to_string(),
				contains_dsl_macro: true,
				skipped: Some(SkipReason::FileWideMarker),
			});
		}

		apply_ignore_markers(content, &mut macros);
		if macros.iter().all(|info| info.should_skip) {
			return Ok(FormatResult {
				content: content.to_string(),
				contains_dsl_macro: true,
				skipped: Some(SkipReason::AllMacrosIgnored),
			});
		}

		let mut result = String::with_capacity(content.len());
		let mut last_end = 0;
		for info in &macros {
			result.push_str(&content[last_end..info.start]);
			if info.should_skip {
				result.push_str(&content[info.start..info.end]);
			} else {
				let original = &content[info.start..info.end];
				match format_macro(
					original,
					info.kind,
					base_indent(content, info.start),
					&self.rustfmt_options,
				) {
					Ok(formatted) => result.push_str(&formatted),
					Err(_) => result.push_str(original),
				}
			}
			last_end = info.end;
		}
		result.push_str(&content[last_end..]);

		Ok(FormatResult {
			content: result,
			contains_dsl_macro: true,
			skipped: None,
		})
	}
}

fn find_dsl_macros(content: &str) -> Result<Vec<MacroInfo>, String> {
	let mut parser = Parser::new();
	let rust_language = tree_sitter_rust::LANGUAGE.into();
	parser
		.set_language(&rust_language)
		.map_err(|e| format!("failed to load tree-sitter-rust grammar: {e}"))?;
	let tree = parser
		.parse(content, None)
		.ok_or_else(|| "failed to parse Rust source with tree-sitter".to_string())?;

	let mut macros = Vec::new();
	collect_macro_nodes(tree.root_node(), content, &mut macros);
	macros.sort_by_key(|info| info.start);
	macros.dedup_by_key(|info| (info.start, info.end));
	Ok(macros)
}

fn collect_macro_nodes(node: Node<'_>, source: &str, macros: &mut Vec<MacroInfo>) {
	if node.kind() == "macro_invocation"
		&& let Ok(text) = node.utf8_text(source.as_bytes())
		&& let Some(kind) = macro_kind(text)
	{
		macros.push(MacroInfo {
			start: node.start_byte(),
			end: node.end_byte(),
			kind,
			should_skip: false,
		});
	}

	let mut cursor = node.walk();
	for child in node.children(&mut cursor) {
		collect_macro_nodes(child, source, macros);
	}
}

fn macro_kind(text: &str) -> Option<MacroKind> {
	let trimmed = text.trim_start();
	for (name, kind) in [
		("page", MacroKind::Page),
		("form", MacroKind::Form),
		("head", MacroKind::Head),
		("style", MacroKind::Style),
	] {
		let Some(rest) = trimmed.strip_prefix(name) else {
			continue;
		};
		if rest.trim_start().starts_with('!') {
			return Some(kind);
		}
	}
	None
}

fn format_macro(
	original: &str,
	kind: MacroKind,
	base_indent: usize,
	rustfmt_options: &RustfmtOptions,
) -> Result<String, String> {
	let parts = split_macro(original, kind)?;
	let direct_page_capture =
		kind == MacroKind::Page && parts.open == '(' && parts.inner.trim_start().starts_with('{');
	let dsl_input = if direct_page_capture {
		format!("|| {}", parts.inner.trim_start())
	} else if parts.open == '{' {
		format!("{}{}{}", parts.open, parts.inner, parts.close)
	} else {
		parts.inner.to_string()
	};
	let mut formatted =
		format_dsl_with_options_preserving_markers(kind, &dsl_input, rustfmt_options)?;
	if direct_page_capture {
		formatted.output = formatted
			.output
			.strip_prefix("|| ")
			.ok_or_else(|| "formatted direct page capture lost its closure prefix".to_string())?
			.to_string();
	}
	let mut formatted_dsl = indent_relative(formatted.output.trim_end(), base_indent);
	restore_protected_rustfmt_islands(&mut formatted_dsl, &formatted.protected_islands);

	let spacer = if parts.open == '(' { "" } else { " " };
	if parts.open == '{' {
		Ok(format!("{}!{}{}", kind.name(), spacer, formatted_dsl))
	} else {
		Ok(format!(
			"{}!{}{}{}{}",
			kind.name(),
			spacer,
			parts.open,
			formatted_dsl,
			parts.close
		))
	}
}

fn split_macro(original: &str, kind: MacroKind) -> Result<MacroParts<'_>, String> {
	let bang = original
		.find('!')
		.ok_or_else(|| format!("{} macro has no !", kind.name()))?;
	let after_bang = &original[bang + 1..];
	let after_ws = after_bang.trim_start();
	let open = after_ws
		.chars()
		.next()
		.ok_or_else(|| format!("{} macro has no delimiter", kind.name()))?;
	let close = match open {
		'(' => ')',
		'{' => '}',
		'[' => ']',
		other => return Err(format!("unsupported macro delimiter: {other}")),
	};
	let open_offset = original.len() - after_ws.len();
	let close_offset = original
		.rfind(close)
		.ok_or_else(|| format!("{} macro has no closing delimiter", kind.name()))?;
	if close_offset <= open_offset {
		return Err(format!("{} macro delimiter range is invalid", kind.name()));
	}

	Ok(MacroParts {
		open,
		close,
		inner: &original[open_offset + open.len_utf8()..close_offset],
	})
}

#[cfg(test)]
fn format_dsl(kind: MacroKind, input: &str) -> Result<String, String> {
	format_dsl_with_options(kind, input, &RustfmtOptions::default())
}

#[cfg(test)]
fn format_dsl_with_options(
	kind: MacroKind,
	input: &str,
	rustfmt_options: &RustfmtOptions,
) -> Result<String, String> {
	let mut formatted = format_dsl_with_options_preserving_markers(kind, input, rustfmt_options)?;
	restore_protected_rustfmt_islands(&mut formatted.output, &formatted.protected_islands);
	Ok(formatted.output)
}

fn format_dsl_with_options_preserving_markers(
	kind: MacroKind,
	input: &str,
	rustfmt_options: &RustfmtOptions,
) -> Result<FormattedDsl, String> {
	validate_dsl_with_tree_sitter(kind, input)?;
	if kind.query().trim().is_empty() {
		return Err(format!("Topiary query for {} DSL is empty", kind.name()));
	}
	let (topiary_input, protected_islands) = if kind == MacroKind::Page {
		protect_topiary_sensitive_page_islands(input)
	} else {
		(input.to_string(), Vec::new())
	};
	let mut output = Vec::new();
	let language = topiary_language(kind)?;
	topiary_formatter_str(
		&topiary_input,
		&mut output,
		&language,
		Operation::Format {
			skip_idempotence: false,
			tolerate_parsing_errors: false,
		},
	)
	.map_err(|e| format!("Topiary failed to format {} DSL: {e}", kind.name()))?;
	let formatted = String::from_utf8(output).map_err(|e| {
		format!(
			"Topiary produced invalid UTF-8 for {} DSL: {e}",
			kind.name()
		)
	})?;
	let mut normalized = normalize_dsl_output(formatted.trim());
	if kind == MacroKind::Page {
		normalized = format_page_rustfmt_islands(&normalized, rustfmt_options);
		normalized = format_page_closure_args(&normalized, rustfmt_options);
	}
	Ok(FormattedDsl {
		output: normalized,
		protected_islands,
	})
}

fn restore_protected_rustfmt_islands(input: &mut String, islands: &[ProtectedRustfmtIsland]) {
	for island in islands {
		*input = input.replace(&island.marker, &island.original);
	}
}

fn normalize_dsl_output(input: &str) -> String {
	input.to_string()
}

fn protect_topiary_sensitive_page_islands(input: &str) -> (String, Vec<ProtectedRustfmtIsland>) {
	let mut parser = Parser::new();
	let language = tree_sitter_reinhardt_page::LANGUAGE.into();
	if parser.set_language(&language).is_err() {
		return (input.to_string(), Vec::new());
	}
	let Some(tree) = parser.parse(input, None) else {
		return (input.to_string(), Vec::new());
	};
	if tree.root_node().has_error() {
		return (input.to_string(), Vec::new());
	}

	let mut ranges = Vec::new();
	collect_topiary_sensitive_rustfmt_island_ranges(tree.root_node(), input, &mut ranges);
	let ranges = non_nested_ranges(ranges);
	if ranges.is_empty() {
		return (input.to_string(), Vec::new());
	}

	let protected: Vec<ProtectedRustfmtIsland> = ranges
		.iter()
		.enumerate()
		.map(|(index, &(start, end))| ProtectedRustfmtIsland {
			marker: protected_island_marker(input, index),
			original: input[start..end].to_string(),
		})
		.collect();

	let mut output = input.to_string();
	for ((start, end), island) in ranges.iter().zip(&protected).rev() {
		output.replace_range(*start..*end, &island.marker);
	}
	(output, protected)
}

fn collect_topiary_sensitive_rustfmt_island_ranges(
	node: Node<'_>,
	source: &str,
	ranges: &mut Vec<(usize, usize)>,
) {
	if node.kind() == "rustfmt_island" {
		let start = node.start_byte();
		let end = node.end_byte();
		if start < end {
			let island = &source[start..end];
			if topiary_sensitive_rustfmt_island(island) {
				if contains_top_level_semicolon(island)
					&& let Some(parent) = node.parent()
					&& parent.kind() == "interpolation"
				{
					ranges.push((parent.start_byte(), parent.end_byte()));
				} else {
					ranges.push((start, end));
				}
			}
		}
		return;
	}

	let mut cursor = node.walk();
	for child in node.children(&mut cursor) {
		collect_topiary_sensitive_rustfmt_island_ranges(child, source, ranges);
	}
}

fn protected_island_marker(input: &str, index: usize) -> String {
	let mut suffix = 0;
	loop {
		let marker = format!("__reinhardt_fmt_protected_island_{index}_{suffix}");
		if !input.contains(&marker) {
			return marker;
		}
		suffix += 1;
	}
}

fn format_page_rustfmt_islands(input: &str, rustfmt_options: &RustfmtOptions) -> String {
	let mut parser = Parser::new();
	let language = tree_sitter_reinhardt_page::LANGUAGE.into();
	if parser.set_language(&language).is_err() {
		return input.to_string();
	}
	let Some(tree) = parser.parse(input, None) else {
		return input.to_string();
	};
	if tree.root_node().has_error() {
		return input.to_string();
	}

	let mut ranges = Vec::new();
	collect_rustfmt_island_ranges(tree.root_node(), &mut ranges);
	let ranges = non_nested_ranges(ranges);

	let mut output = input.to_string();
	for (start, end) in ranges.into_iter().rev() {
		let island = &input[start..end];
		let line_indent = line_indent_for_offset(input, start);
		if let Some(formatted) = format_rustfmt_island(island, line_indent, rustfmt_options) {
			output.replace_range(start..end, &formatted);
		}
	}
	output
}

fn format_page_closure_args(input: &str, rustfmt_options: &RustfmtOptions) -> String {
	let mut parser = Parser::new();
	let language = tree_sitter_reinhardt_page::LANGUAGE.into();
	if parser.set_language(&language).is_err() {
		return input.to_string();
	}
	let Some(tree) = parser.parse(input, None) else {
		return input.to_string();
	};
	if tree.root_node().has_error() {
		return input.to_string();
	}

	let mut ranges = Vec::new();
	collect_page_closure_arg_ranges(tree.root_node(), &mut ranges);
	let ranges = non_nested_ranges(ranges);

	let mut output = input.to_string();
	for (start, end) in ranges.into_iter().rev() {
		let closure_args = &input[start..end];
		let line_indent = line_indent_for_offset(input, start);
		if let Some(formatted) = format_closure_args(closure_args, line_indent, rustfmt_options) {
			output.replace_range(start..end, &formatted);
		}
	}
	output
}

fn collect_page_closure_arg_ranges(node: Node<'_>, ranges: &mut Vec<(usize, usize)>) {
	if node.kind() == "page_closure" {
		let mut cursor = node.walk();
		for child in node.children(&mut cursor) {
			if child.kind() == "closure_args" {
				let start = child.start_byte();
				let end = child.end_byte();
				if start < end {
					ranges.push((start, end));
				}
				break;
			}
		}
		return;
	}

	let mut cursor = node.walk();
	for child in node.children(&mut cursor) {
		collect_page_closure_arg_ranges(child, ranges);
	}
}

fn format_closure_args(
	closure_args: &str,
	line_indent: &str,
	rustfmt_options: &RustfmtOptions,
) -> Option<String> {
	let wrapped = format!("fn main() {{ let __reinhardt_fmt = {closure_args} (); }}\n");
	let mut cmd = Command::new("rustfmt");
	rustfmt_options.apply_to_island_command(&mut cmd);
	let mut child = cmd
		.arg("--emit=stdout")
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::null())
		.spawn()
		.ok()?;
	let Some(mut stdin) = child.stdin.take() else {
		let _ = child.kill();
		let _ = child.wait();
		return None;
	};
	if stdin.write_all(wrapped.as_bytes()).is_err() {
		drop(stdin);
		let _ = child.kill();
		let _ = child.wait();
		return None;
	}
	drop(stdin);

	let output = child.wait_with_output().ok()?;
	if !output.status.success() {
		return None;
	}
	let formatted_wrapper = String::from_utf8(output.stdout).ok()?;
	let formatted = unwrap_rustfmt_closure_args(&formatted_wrapper)?;
	Some(reindent_multiline_closure_args(formatted, line_indent))
}

fn collect_rustfmt_island_ranges(node: Node<'_>, ranges: &mut Vec<(usize, usize)>) {
	if node.kind() == "rustfmt_island" {
		let start = node.start_byte();
		let end = node.end_byte();
		if start < end {
			ranges.push((start, end));
		}
		return;
	}

	let mut cursor = node.walk();
	for child in node.children(&mut cursor) {
		collect_rustfmt_island_ranges(child, ranges);
	}
}

fn non_nested_ranges(mut ranges: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
	ranges.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| right.1.cmp(&left.1)));
	let mut kept = Vec::with_capacity(ranges.len());
	for range in ranges {
		let is_nested = kept
			.iter()
			.any(|outer: &(usize, usize)| outer.0 <= range.0 && range.1 <= outer.1);
		if !is_nested {
			kept.push(range);
		}
	}
	kept.sort_by_key(|range| range.0);
	kept
}

fn format_rustfmt_island(
	island: &str,
	line_indent: &str,
	rustfmt_options: &RustfmtOptions,
) -> Option<String> {
	if topiary_sensitive_rustfmt_island(island) {
		return None;
	}
	let wrapped = format!("fn main() {{ let __reinhardt_fmt = {island}; }}\n");
	let mut cmd = Command::new("rustfmt");
	rustfmt_options.apply_to_island_command(&mut cmd);
	let mut child = cmd
		.arg("--emit=stdout")
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::null())
		.spawn()
		.ok()?;
	let Some(mut stdin) = child.stdin.take() else {
		let _ = child.kill();
		let _ = child.wait();
		return None;
	};
	if stdin.write_all(wrapped.as_bytes()).is_err() {
		drop(stdin);
		let _ = child.kill();
		let _ = child.wait();
		return None;
	}
	drop(stdin);

	let output = child.wait_with_output().ok()?;
	if !output.status.success() {
		return None;
	}
	let formatted_wrapper = String::from_utf8(output.stdout).ok()?;
	let formatted = unwrap_rustfmt_island(&formatted_wrapper)?;
	Some(reindent_multiline_island(formatted, line_indent))
}

fn contains_rust_comment_marker(input: &str) -> bool {
	input.contains("//") || input.contains("/*")
}

fn topiary_sensitive_rustfmt_island(input: &str) -> bool {
	contains_rust_comment_marker(input) || contains_top_level_semicolon(input)
}

fn contains_top_level_semicolon(input: &str) -> bool {
	let chars: Vec<char> = input.chars().collect();
	let mut index = 0;
	let mut paren_depth = 0usize;
	let mut bracket_depth = 0usize;
	let mut brace_depth = 0usize;

	while index < chars.len() {
		match chars[index] {
			'"' => skip_string_literal(&chars, &mut index),
			'\'' => skip_char_or_lifetime(&chars, &mut index),
			'r' if skip_raw_string_literal(&chars, &mut index) => {}
			'/' if index + 1 < chars.len() && chars[index + 1] == '/' => {
				index += 2;
				while index < chars.len() && chars[index] != '\n' {
					index += 1;
				}
			}
			'/' if index + 1 < chars.len() && chars[index + 1] == '*' => {
				index += 2;
				while index + 1 < chars.len() && !(chars[index] == '*' && chars[index + 1] == '/') {
					index += 1;
				}
				index = (index + 2).min(chars.len());
			}
			'(' => {
				paren_depth += 1;
				index += 1;
			}
			')' => {
				paren_depth = paren_depth.saturating_sub(1);
				index += 1;
			}
			'[' => {
				bracket_depth += 1;
				index += 1;
			}
			']' => {
				bracket_depth = bracket_depth.saturating_sub(1);
				index += 1;
			}
			'{' => {
				brace_depth += 1;
				index += 1;
			}
			'}' => {
				brace_depth = brace_depth.saturating_sub(1);
				index += 1;
			}
			';' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => return true,
			_ => index += 1,
		}
	}

	false
}

fn skip_string_literal(chars: &[char], index: &mut usize) {
	*index += 1;
	while *index < chars.len() {
		let current = chars[*index];
		*index += 1;
		if current == '\\' && *index < chars.len() {
			*index += 1;
			continue;
		}
		if current == '"' {
			break;
		}
	}
}

fn skip_char_or_lifetime(chars: &[char], index: &mut usize) {
	*index += 1;
	if *index < chars.len() && is_ident_start(chars[*index]) {
		*index += 1;
		while *index < chars.len() && is_ident_continue(chars[*index]) {
			*index += 1;
		}
		if *index >= chars.len() || chars[*index] != '\'' {
			return;
		}
	}
	while *index < chars.len() {
		let current = chars[*index];
		*index += 1;
		if current == '\\' && *index < chars.len() {
			*index += 1;
			continue;
		}
		if current == '\'' {
			break;
		}
	}
}

fn is_ident_start(ch: char) -> bool {
	ch == '_' || ch.is_ascii_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
	is_ident_start(ch) || ch.is_ascii_digit()
}

fn skip_raw_string_literal(chars: &[char], index: &mut usize) -> bool {
	let start = *index;
	let mut cursor = start + 1;
	let mut hashes = 0usize;
	while cursor < chars.len() && chars[cursor] == '#' {
		hashes += 1;
		cursor += 1;
	}
	if cursor >= chars.len() || chars[cursor] != '"' {
		return false;
	}
	cursor += 1;
	while cursor < chars.len() {
		if chars[cursor] == '"' {
			let mut matched_hashes = 0usize;
			while matched_hashes < hashes
				&& cursor + 1 + matched_hashes < chars.len()
				&& chars[cursor + 1 + matched_hashes] == '#'
			{
				matched_hashes += 1;
			}
			if matched_hashes == hashes {
				*index = cursor + 1 + matched_hashes;
				return true;
			}
		}
		cursor += 1;
	}
	*index = chars.len();
	true
}

fn unwrap_rustfmt_island(formatted_wrapper: &str) -> Option<&str> {
	let mut parser = Parser::new();
	let language = tree_sitter_rust::LANGUAGE.into();
	parser.set_language(&language).ok()?;
	let tree = parser.parse(formatted_wrapper, None)?;
	if tree.root_node().has_error() {
		return None;
	}
	let declaration = find_first_node_kind(tree.root_node(), "let_declaration")?;
	let value = declaration.child_by_field_name("value")?;
	value.utf8_text(formatted_wrapper.as_bytes()).ok()
}

fn unwrap_rustfmt_closure_args(formatted_wrapper: &str) -> Option<&str> {
	let mut parser = Parser::new();
	let language = tree_sitter_rust::LANGUAGE.into();
	parser.set_language(&language).ok()?;
	let tree = parser.parse(formatted_wrapper, None)?;
	if tree.root_node().has_error() {
		return None;
	}
	let closure = find_first_node_kind(tree.root_node(), "closure_expression")?;
	let parameters = closure.child_by_field_name("parameters")?;
	parameters.utf8_text(formatted_wrapper.as_bytes()).ok()
}

fn find_first_node_kind<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
	if node.kind() == kind {
		return Some(node);
	}
	let mut cursor = node.walk();
	for child in node.children(&mut cursor) {
		if let Some(found) = find_first_node_kind(child, kind) {
			return Some(found);
		}
	}
	None
}

fn reindent_multiline_island(input: &str, line_indent: &str) -> String {
	let mut lines: Vec<&str> = input.lines().collect();
	if lines.len() <= 1 {
		return input.to_string();
	}
	let strip_indent = common_continuation_indent(&lines[1..]);
	let mut result = String::from(lines.remove(0));
	for line in lines {
		result.push('\n');
		if !line.is_empty() {
			result.push_str(line_indent);
		}
		result.push_str(strip_leading_indent(line, strip_indent));
	}
	result
}

fn reindent_multiline_closure_args(input: &str, line_indent: &str) -> String {
	let mut lines = input.lines();
	let Some(first) = lines.next() else {
		return String::new();
	};
	let mut result = first.to_string();
	for line in lines {
		result.push('\n');
		let trimmed = line.trim_start_matches([' ', '\t']);
		if !trimmed.is_empty() {
			result.push_str(line_indent);
			result.push(' ');
			result.push_str(trimmed);
		}
	}
	result
}

fn common_continuation_indent(lines: &[&str]) -> usize {
	lines
		.iter()
		.filter(|line| !line.is_empty())
		.map(|line| line.len() - line.trim_start_matches([' ', '\t']).len())
		.min()
		.unwrap_or(0)
}

fn strip_leading_indent(line: &str, count: usize) -> &str {
	let mut bytes = 0;
	for ch in line.chars().take(count) {
		if ch != ' ' && ch != '\t' {
			break;
		}
		bytes += ch.len_utf8();
	}
	&line[bytes..]
}

fn line_indent_for_offset(input: &str, offset: usize) -> &str {
	let line_start = input[..offset].rfind('\n').map_or(0, |pos| pos + 1);
	let line_prefix = &input[line_start..offset];
	let indent_end = line_prefix
		.find(|ch: char| ch != ' ' && ch != '\t')
		.map_or(offset, |pos| line_start + pos);
	&input[line_start..indent_end]
}

fn topiary_language(kind: MacroKind) -> Result<TopiaryLanguage, String> {
	let grammar: TopiaryTreeSitterLanguage = match kind {
		MacroKind::Page => tree_sitter_reinhardt_page::LANGUAGE.into(),
		MacroKind::Form => tree_sitter_reinhardt_form::LANGUAGE.into(),
		MacroKind::Head => tree_sitter_reinhardt_head::LANGUAGE.into(),
		MacroKind::Style => tree_sitter_reinhardt_style::LANGUAGE.into(),
	};
	let query = TopiaryQuery::new(&grammar, kind.query())
		.map_err(|e| format!("invalid Topiary query for {} DSL: {e}", kind.name()))?;
	Ok(TopiaryLanguage {
		name: format!("reinhardt_{}", kind.name()),
		query,
		grammar,
		indent: Some("\t".to_string()),
	})
}

fn validate_dsl_with_tree_sitter(kind: MacroKind, input: &str) -> Result<(), String> {
	let mut parser = Parser::new();
	let language = kind.grammar();
	parser
		.set_language(&language)
		.map_err(|e| format!("failed to load {} DSL grammar: {e}", kind.name()))?;
	let tree = parser
		.parse(input, None)
		.ok_or_else(|| format!("failed to parse {} DSL", kind.name()))?;
	if tree.root_node().has_error() {
		return Err(format!("{} DSL parse contains ERROR nodes", kind.name()));
	}
	Ok(())
}

fn indent_relative(input: &str, base_indent: usize) -> String {
	if base_indent == 0 {
		return input.to_string();
	}
	let prefix = "\t".repeat(base_indent);
	let mut lines = input.lines();
	let Some(first) = lines.next() else {
		return String::new();
	};
	let mut result = first.to_string();
	for line in lines {
		result.push('\n');
		if !line.is_empty() {
			result.push_str(&prefix);
		}
		result.push_str(line);
	}
	result
}

fn base_indent(content: &str, offset: usize) -> usize {
	let line_start = content[..offset].rfind('\n').map_or(0, |pos| pos + 1);
	content[line_start..offset]
		.chars()
		.filter(|&ch| ch == '\t')
		.count()
}

fn apply_ignore_markers(content: &str, macros: &mut [MacroInfo]) {
	let line_starts = line_starts(content);
	for info in macros {
		let line_index = line_index_for_offset(&line_starts, info.start);
		info.should_skip = has_line_ignore(content, &line_starts, line_index)
			|| is_in_disabled_range(content, line_index);
	}
}

fn line_starts(content: &str) -> Vec<usize> {
	let mut starts = vec![0];
	for (index, byte) in content.bytes().enumerate() {
		if byte == b'\n' {
			starts.push(index + 1);
		}
	}
	starts
}

fn line_index_for_offset(starts: &[usize], offset: usize) -> usize {
	match starts.binary_search(&offset) {
		Ok(index) => index,
		Err(index) => index.saturating_sub(1),
	}
}

fn has_line_ignore(content: &str, starts: &[usize], line_index: usize) -> bool {
	if line_index == 0 {
		return false;
	}
	let previous = line(content, starts, line_index - 1);
	marker_matches(previous, "reinhardt-fmt:ignore")
}

fn is_in_disabled_range(content: &str, line_index: usize) -> bool {
	let mut disabled = false;
	for line in content.lines().take(line_index) {
		if marker_matches(line, "reinhardt-fmt:off") {
			disabled = true;
		} else if marker_matches(line, "reinhardt-fmt:on") {
			disabled = false;
		}
	}
	disabled
}

fn line<'a>(content: &'a str, starts: &[usize], line_index: usize) -> &'a str {
	let start = starts[line_index];
	let end = starts.get(line_index + 1).copied().unwrap_or(content.len());
	content[start..end].trim_end_matches('\n')
}

fn marker_matches(line: &str, compact_marker: &str) -> bool {
	let trimmed = line.trim_start();
	if !trimmed.starts_with("//") {
		return false;
	}
	trimmed
		.chars()
		.filter(|ch| !ch.is_whitespace())
		.collect::<String>()
		.contains(compact_marker)
}

fn rustfmt_skip_attr_matches(line: &str) -> bool {
	let trimmed = line.trim_start();
	if !trimmed.starts_with("#![") {
		return false;
	}
	let compact: String = trimmed.chars().filter(|ch| !ch.is_whitespace()).collect();
	compact.starts_with("#![rustfmt::skip]")
}

#[cfg(test)]
mod tests {
	use rstest::rstest;

	use super::*;

	// -----------------------------------------------------------------------
	// Existing tests (converted from #[test] to #[rstest])
	// -----------------------------------------------------------------------

	#[rstest]
	fn tree_sitter_detection_ignores_strings() {
		// Arrange
		let source = r#"fn main() {
	let literal = "page!(|| { div { \"x\" } })";
	let view = page!(|| { div { "x" } });
}"#;

		// Act
		let macros = find_dsl_macros(source).expect("parse source");

		// Assert
		assert_eq!(macros.len(), 1);
		assert_eq!(macros[0].kind, MacroKind::Page);
	}

	#[rstest]
	fn formats_page_macro_with_topiary_query() {
		// Arrange
		let formatter = FormatEngine::new();
		let source = r#"fn main() {
	let view = page!(|| { div { "x" } });
}"#;

		// Act
		let result = formatter.format(source).expect("format source");

		// Assert
		assert_eq!(
			result.content,
			"fn main() {\n\tlet view = page!(|| {\n\t\tdiv { \"x\" }\n\t});\n}"
		);
		assert_eq!(result.skipped, None);
	}

	#[rstest]
	fn formats_page_macro_with_direct_body() {
		// Arrange
		let formatter = FormatEngine::new();
		let source = r#"fn main() {
	let view = page!({ div { "x" } });
}"#;

		// Act
		let result = formatter.format(source).expect("format source");

		// Assert
		assert_eq!(
			result.content,
			"fn main() {\n\tlet view = page!({\n\t\tdiv { \"x\" }\n\t});\n}"
		);
		assert_eq!(result.skipped, None);
	}

	#[rstest]
	fn formats_page_dsl_directly_with_topiary_query() {
		// Act
		let formatted =
			format_dsl(MacroKind::Page, r#"|| { div { "x" } }"#).expect("format page DSL");

		// Assert
		assert_eq!(formatted, "|| {\n\tdiv { \"x\" }\n}");
	}

	#[rstest]
	fn formats_long_page_closure_args_as_multiline() {
		// Arrange
		let input = concat!(
			"|project_id: i64, ",
			"small_box_id: i64, ",
			"instruction_id: String, ",
			"generate_pending_action: VersionAction, ",
			"generate_click: ClickHandler, ",
			"retake_pending_action: VersionAction, ",
			"retake_click: ClickHandler, ",
			"accept_pending_action: VersionAction, ",
			"accept_click: ClickHandler, ",
			"compare_pending_action: CompareAction, ",
			"compare_click: ClickHandler, ",
			"manuscript_badge_handles: ManuscriptResultHandles, ",
			"manuscript_summary_handles: ManuscriptResultHandles| ",
			r#"{ section { class: "grid gap-5 p-6", } }"#,
		);

		// Act
		let formatted = format_dsl(MacroKind::Page, input).expect("format page DSL");
		let second = format_dsl(MacroKind::Page, &formatted).expect("format page DSL again");

		// Assert
		assert!(
			formatted.contains("|project_id: i64,\n"),
			"long closure args should wrap after the first parameter: {formatted}"
		);
		assert!(
			formatted.contains("\n small_box_id: i64,"),
			"wrapped closure args should keep later parameters on separate lines: {formatted}"
		);
		let small_box_line = formatted
			.lines()
			.find(|line| line.contains("small_box_id: i64,"))
			.expect("small_box_id line");
		assert_eq!(
			small_box_line, " small_box_id: i64,",
			"wrapped closure args should not preserve rustfmt alignment padding: {formatted}"
		);
		assert!(
			!formatted.contains("project_id: i64, small_box_id: i64, instruction_id: String"),
			"long closure args should not stay on one line: {formatted}"
		);
		assert_eq!(second, formatted);

		let hard_tabs_options = RustfmtOptions {
			config: Some("hard_tabs=true".to_string()),
			..RustfmtOptions::default()
		};
		let hard_tabs_formatted =
			format_dsl_with_options(MacroKind::Page, input, &hard_tabs_options)
				.expect("format page DSL with hard tabs");
		let hard_tabs_second =
			format_dsl_with_options(MacroKind::Page, &hard_tabs_formatted, &hard_tabs_options)
				.expect("format page DSL with hard tabs again");
		let hard_tabs_small_box_line = hard_tabs_formatted
			.lines()
			.find(|line| line.contains("small_box_id: i64,"))
			.expect("hard-tabs small_box_id line");
		assert_eq!(
			hard_tabs_small_box_line, " small_box_id: i64,",
			"wrapped closure args should normalize hard-tabs rustfmt alignment padding: {hard_tabs_formatted}"
		);
		assert_eq!(hard_tabs_second, hard_tabs_formatted);
	}

	#[rstest]
	fn formats_page_direct_body_dsl_directly_with_topiary_query() {
		// Act
		let formatted = format_dsl(MacroKind::Page, r#"{ div { "x" } }"#).expect("format page DSL");

		// Assert
		assert_eq!(formatted, "{\n\tdiv { \"x\" }\n}");
	}

	#[rstest]
	fn formats_page_semantic_wrappers_preserve_control_flow_spacing() {
		// Act
		let formatted = format_dsl(MacroKind::Page, r#"|| { if show { div { "x" } } }"#)
			.expect("format page DSL");

		// Assert
		assert_eq!(formatted, "|| {\n\tif show {\n\t\tdiv { \"x\" }\n\t}\n}");
	}

	#[rstest]
	fn formats_page_semantic_attribute_blocks_without_extra_indent() {
		// Act
		let formatted = format_dsl(
			MacroKind::Page,
			r#"|| { div { class: "container", "Styled content" } }"#,
		)
		.expect("format page DSL");

		// Assert
		assert_eq!(
			formatted,
			"|| {\n\tdiv {\n\t\tclass: \"container\",\n\t\t\"Styled content\"\n\t}\n}"
		);
	}

	#[rstest]
	fn formats_page_semantic_siblings_with_hardline_separators() {
		// Act
		let formatted = format_dsl(
			MacroKind::Page,
			r#"|| { div { h1 { { title } } { add_link } div { { summary } } } }"#,
		)
		.expect("format page DSL");

		// Assert
		assert!(
			formatted.contains("h1 { { title } }\n\t\t{ add_link }\n\t\tdiv { { summary } }"),
			"semantic siblings should stay line-separated: {formatted}"
		);
		assert!(
			!formatted.contains("}div"),
			"semantic element siblings must not concatenate: {formatted}"
		);
	}

	#[rstest]
	fn preserves_separator_between_text_literal_and_following_fragment() {
		for kind in [MacroKind::Page, MacroKind::Form, MacroKind::Head] {
			// Act
			let formatted = format_dsl(kind, r#"|| { label { "hello"span { "world" } } }"#)
				.expect("format DSL");

			// Assert
			assert!(
				formatted.contains(r#""hello" span {"#),
				"{kind:?} formatter should keep a separator before the following fragment: {formatted}"
			);
			assert!(
				!formatted.contains(r#""hello"span"#),
				"{kind:?} formatter must not concatenate text literal and fragment: {formatted}"
			);
		}
	}

	#[rstest]
	fn preserves_ignored_macro() {
		// Arrange
		let formatter = FormatEngine::new();
		let source = r#"fn main() {
	// reinhardt-fmt: ignore
	let view = page!(|| { div { "x" } });
}"#;

		// Act
		let result = formatter.format(source).expect("format source");

		// Assert
		assert_eq!(result.skipped, Some(SkipReason::AllMacrosIgnored));
		assert_eq!(result.content, source);
	}

	#[rstest]
	fn formats_form_brace_macro() {
		// Arrange
		let formatter = FormatEngine::new();
		let source = r#"fn main() {
	let form = form! { name: "User", fields { email { label: "Email", } } };
}"#;

		// Act
		let result = formatter.format(source).expect("format source");

		// Assert
		assert!(result.content.contains("form! {"));
		assert!(result.content.contains("name:"));
		assert!(result.content.contains("\n\t\tfields {"));
	}

	#[rstest]
	fn formats_head_macro() {
		// Arrange
		let formatter = FormatEngine::new();
		let source = r#"fn main() {
	let head = head!(|| { title { "Polls" } });
}"#;

		// Act
		let result = formatter.format(source).expect("format source");

		// Assert
		assert!(result.content.contains("head!(|| {"));
		assert!(result.content.contains("\n\t\ttitle {"));
	}

	#[rstest]
	fn preserves_rust_spacing_and_comments_inside_fragments() {
		// Arrange
		let formatter = FormatEngine::new();
		let source = r#"fn main() {
	let view = page!(|| { if let Some(e) = err.clone() { div { // keep words (Edit / Delete)
		"x"
	} } for item in items { div {} } });
	let form = form! { watch: { result: |form| { match user { Some(Some(ref u)) => u.id, _ => 0, } }, } };
}"#;

		// Act
		let result = formatter.format(source).expect("format source");
		let second = formatter
			.format(&result.content)
			.expect("format source again");

		// Assert
		assert_eq!(second.content, result.content);
		assert!(result.content.contains("if let Some(e) = err.clone() {"));
		assert!(result.content.contains("Some(Some(ref u)) => u.id"));
		assert!(result.content.contains("// keep words (Edit / Delete)"));
		assert!(!result.content.contains("}for"));
	}

	// -----------------------------------------------------------------------
	// marker_matches tests
	// -----------------------------------------------------------------------

	#[rstest]
	fn marker_matches_in_line_comment() {
		// Arrange
		let line = "// reinhardt-fmt: off";

		// Act
		let result = marker_matches(line, "reinhardt-fmt:off");

		// Assert
		assert!(result);
	}

	#[rstest]
	fn marker_matches_ignores_string_literal() {
		// Arrange
		let line = r#"let s = "reinhardt-fmt: off";"#;

		// Act
		let result = marker_matches(line, "reinhardt-fmt:off");

		// Assert
		assert!(!result);
	}

	#[rstest]
	fn marker_matches_ignores_code_line() {
		// Arrange
		let line = "let off = reinhardt_fmt_off;";

		// Act
		let result = marker_matches(line, "reinhardt-fmt:off");

		// Assert
		assert!(!result);
	}

	#[rstest]
	fn marker_matches_with_leading_whitespace_comment() {
		// Arrange: indented comment line
		let line = "\t\t// reinhardt-fmt: ignore";

		// Act
		let result = marker_matches(line, "reinhardt-fmt:ignore");

		// Assert
		assert!(result);
	}

	#[rstest]
	fn marker_matches_with_extra_spaces_in_marker() {
		// Arrange: spaces around colon
		let line = "//  reinhardt-fmt :  off";

		// Act
		let result = marker_matches(line, "reinhardt-fmt:off");

		// Assert: whitespace is stripped before comparison
		assert!(result);
	}

	#[rstest]
	fn marker_matches_returns_false_for_empty_line() {
		// Arrange
		let line = "";

		// Act
		let result = marker_matches(line, "reinhardt-fmt:off");

		// Assert
		assert!(!result);
	}

	// -----------------------------------------------------------------------
	// rustfmt_skip_attr_matches tests
	// -----------------------------------------------------------------------

	#[rstest]
	fn rustfmt_skip_attr_matches_true_for_actual_attribute() {
		// Arrange
		let line = "#![rustfmt::skip]";

		// Act
		let result = rustfmt_skip_attr_matches(line);

		// Assert
		assert!(result);
	}

	#[rstest]
	fn rustfmt_skip_attr_matches_true_for_indented_attribute() {
		// Arrange
		let line = "  #![rustfmt::skip]";

		// Act
		let result = rustfmt_skip_attr_matches(line);

		// Assert
		assert!(result);
	}

	#[rstest]
	fn rustfmt_skip_attr_matches_false_for_line_comment() {
		// Arrange
		let line = "// #![rustfmt::skip]";

		// Act
		let result = rustfmt_skip_attr_matches(line);

		// Assert
		assert!(!result);
	}

	#[rstest]
	fn rustfmt_skip_attr_matches_false_for_block_comment() {
		// Arrange
		let line = "/* #![rustfmt::skip] */";

		// Act
		let result = rustfmt_skip_attr_matches(line);

		// Assert
		assert!(!result);
	}

	#[rstest]
	fn rustfmt_skip_attr_matches_false_for_empty_line() {
		// Arrange
		let line = "";

		// Act
		let result = rustfmt_skip_attr_matches(line);

		// Assert
		assert!(!result);
	}

	// -----------------------------------------------------------------------
	// has_ignore_all_marker tests
	// -----------------------------------------------------------------------

	#[rstest]
	fn has_ignore_all_marker_in_first_lines() {
		// Arrange
		let formatter = FormatEngine::new();
		let content = "// reinhardt-fmt: ignore-all\nfn main() {}";

		// Act
		let result = formatter.has_ignore_all_marker(content);

		// Assert
		assert!(result);
	}

	#[rstest]
	fn has_ignore_all_marker_absent() {
		// Arrange
		let formatter = FormatEngine::new();
		let content = "fn main() {\n\tpage!(|| { div { \"x\" } });\n}";

		// Act
		let result = formatter.has_ignore_all_marker(content);

		// Assert
		assert!(!result);
	}

	#[rstest]
	fn has_ignore_all_marker_after_code_is_ignored() {
		// Arrange: marker appears after a non-comment, non-empty line, so
		// the `take_while` predicate stops before reaching it
		let formatter = FormatEngine::new();
		let mut lines: Vec<String> = Vec::new();
		lines.push("fn main() {}".to_string());
		for _ in 0..50 {
			lines.push(String::new());
		}
		lines.push("// reinhardt-fmt: ignore-all".to_string());
		let content = lines.join("\n");

		// Act
		let result = formatter.has_ignore_all_marker(&content);

		// Assert: marker is unreachable because the predicate stops at the
		// first non-comment, non-empty line (line 0: "fn main() {}")
		assert!(!result);
	}

	#[rstest]
	fn has_ignore_all_marker_among_comments() {
		// Arrange: all lines before the marker are comments or empty
		let formatter = FormatEngine::new();
		let content = "\
// Module doc comment
// More comments

// reinhardt-fmt: ignore-all
fn main() {}";

		// Act
		let result = formatter.has_ignore_all_marker(content);

		// Assert
		assert!(result);
	}

	#[rstest]
	fn has_ignore_all_marker_accepts_rustfmt_skip_inner_attr() {
		// Arrange
		let formatter = FormatEngine::new();
		let content = "#![rustfmt::skip]\nfn main() {\n\tpage!(|| { div { \"x\" } });\n}";

		// Act
		let result = formatter.has_ignore_all_marker(content);

		// Assert
		assert!(result);
	}

	#[rstest]
	fn has_ignore_all_marker_rejects_commented_out_rustfmt_skip() {
		// Arrange
		let formatter = FormatEngine::new();
		let content = "// #![rustfmt::skip]\nfn main() {\n\tpage!(|| { div { \"x\" } });\n}";

		// Act
		let result = formatter.has_ignore_all_marker(content);

		// Assert
		assert!(!result);
	}

	// -----------------------------------------------------------------------
	// Format engine: no DSL macros
	// -----------------------------------------------------------------------

	#[rstest]
	fn no_dsl_macros_returns_unchanged_content() {
		// Arrange
		let formatter = FormatEngine::new();
		let source = "fn main() {\n\tprintln!(\"hello\");\n}";

		// Act
		let result = formatter.format(source).expect("format source");

		// Assert
		assert_eq!(result.content, source);
		assert!(!result.contains_dsl_macro);
		assert_eq!(result.skipped, None);
	}

	// -----------------------------------------------------------------------
	// Format engine: multiple macros
	// -----------------------------------------------------------------------

	#[rstest]
	fn formats_multiple_macros_in_one_file() {
		// Arrange
		let formatter = FormatEngine::new();
		let source = r#"fn main() {
		let view = page!(|| { div { "hello" } });
	let h = head!(|| { title { "Title" } });
}"#;

		// Act
		let result = formatter.format(source).expect("format source");

		// Assert: both macros should be formatted (multi-line expansion)
		assert!(
			result.content.contains("page!(|| {"),
			"page macro should be formatted: {}",
			result.content
		);
		assert!(
			result.content.contains("head!(|| {"),
			"head macro should be formatted: {}",
			result.content
		);
		assert!(result.contains_dsl_macro);
		assert_eq!(result.skipped, None);
	}

	#[rstest]
	fn protected_page_islands_preserve_multiline_raw_string_bytes() {
		// Arrange
		let formatter = FormatEngine::new();
		let protected = "/* keep */ r#\"line1\n\t\t\t\t    line2\"#";
		let source = format!(
			"fn main() {{\n\tlet view = page!(|| {{\n\t\tdiv {{\n\t\t\ttitle: {protected},\n\t\t\t\"x\"\n\t\t}}\n\t}});\n}}"
		);

		// Act
		let result = formatter.format(&source).expect("format source");

		// Assert
		assert!(
			result.content.contains(protected),
			"protected Rust island should preserve raw string bytes: {}",
			result.content
		);
	}

	// -----------------------------------------------------------------------
	// Format engine: file-wide ignore marker
	// -----------------------------------------------------------------------

	#[rstest]
	fn file_wide_ignore_marker_skips_formatting() {
		// Arrange
		let formatter = FormatEngine::new();
		let source =
			"// reinhardt-fmt: ignore-all\nfn main() {\n\tlet v = page!(|| { div { \"x\" } });\n}";

		// Act
		let result = formatter.format(source).expect("format source");

		// Assert
		assert_eq!(result.skipped, Some(SkipReason::FileWideMarker));
		assert!(result.contains_dsl_macro);
		assert_eq!(result.content, source);
	}

	// -----------------------------------------------------------------------
	// Format engine: individual ignore markers
	// -----------------------------------------------------------------------

	#[rstest]
	fn individual_ignore_preserves_targeted_macro_only() {
		// Arrange: two macros, only the first has an ignore marker
		let formatter = FormatEngine::new();
		let source = r#"fn main() {
	// reinhardt-fmt: ignore
	let view = page!(|| { div { "x" } });
	let h = head!(|| { title { "Title" } });
}"#;

		// Act
		let result = formatter.format(source).expect("format source");

		// Assert: first macro preserved verbatim, second formatted
		assert!(
			result.content.contains(r#"page!(|| { div { "x" } })"#),
			"ignored macro should be preserved verbatim: {}",
			result.content
		);
		assert!(
			result.content.contains("\n\t\ttitle {"),
			"non-ignored macro should be formatted: {}",
			result.content
		);
		assert_eq!(result.skipped, None);
	}

	// -----------------------------------------------------------------------
	// Format engine: off/on range markers
	// -----------------------------------------------------------------------

	#[rstest]
	fn off_on_range_preserves_macros_in_disabled_region() {
		// Arrange
		let formatter = FormatEngine::new();
		let source = r#"fn main() {
	// reinhardt-fmt: off
	let view = page!(|| { div { "x" } });
	// reinhardt-fmt: on
	let h = head!(|| { title { "Title" } });
}"#;

		// Act
		let result = formatter.format(source).expect("format source");

		// Assert: macro inside off/on region is preserved, outside is formatted
		assert!(
			result.content.contains(r#"page!(|| { div { "x" } })"#),
			"macro in disabled region should be preserved: {}",
			result.content
		);
		assert!(
			result.content.contains("\n\t\ttitle {"),
			"macro outside disabled region should be formatted: {}",
			result.content
		);
		assert_eq!(result.skipped, None);
	}

	// -----------------------------------------------------------------------
	// Format engine: idempotency
	// -----------------------------------------------------------------------

	#[rstest]
	fn formatting_is_idempotent() {
		// Arrange
		let formatter = FormatEngine::new();
		let source = r#"fn main() {
	let view = page!(|| { div { "x" } });
	let h = head!(|| { title { "T" } });
}"#;

		// Act
		let first = formatter.format(source).expect("first format");
		let second = formatter.format(&first.content).expect("second format");

		// Assert
		assert_eq!(first.content, second.content);
	}

	#[rstest]
	fn direct_page_capture_starting_with_interpolation_is_idempotent() {
		// Arrange
		let options = RustfmtOptions {
			config: Some("hard_tabs=true".to_string()),
			..RustfmtOptions::default()
		};
		let formatter = FormatEngine::with_rustfmt_options(options.clone());
		let source = r#"fn render() -> Page {
	Page::reactive(move || {
		page!({
			{ success_view }
			{ error_view }
			form {
				class: "stack",
				button {
					type: "submit",
					"Save"
				}
			}
		})
	})
}"#;
		// Act
		let first_dsl = formatter.format(source).expect("first DSL format");
		let first = crate::run_rustfmt(&first_dsl.content, &options).expect("first rustfmt");
		let second_dsl = formatter.format(&first).expect("second DSL format");
		let second = crate::run_rustfmt(&second_dsl.content, &options).expect("second rustfmt");

		// Assert
		assert_eq!(
			first, second,
			"first DSL:\n{}\nsecond DSL:\n{}",
			first_dsl.content, second_dsl.content
		);
	}

	// -----------------------------------------------------------------------
	// find_dsl_macros tests
	// -----------------------------------------------------------------------

	#[rstest]
	fn find_dsl_macros_empty_for_no_macros() {
		// Arrange
		let source = "fn main() {\n\tprintln!(\"hello\");\n}";

		// Act
		let macros = find_dsl_macros(source).expect("parse source");

		// Assert
		assert_eq!(macros.len(), 0);
	}

	#[rstest]
	fn find_dsl_macros_detects_all_three_kinds() {
		// Arrange
		let source = r#"fn main() {
	let p = page!(|| { div {} });
	let f = form! { name: "F", fields { x { label: "X", } } };
	let h = head!(|| { title { "T" } });
}"#;

		// Act
		let macros = find_dsl_macros(source).expect("parse source");

		// Assert
		assert_eq!(macros.len(), 3);
		let kinds: Vec<MacroKind> = macros.iter().map(|m| m.kind).collect();
		assert!(kinds.contains(&MacroKind::Page));
		assert!(kinds.contains(&MacroKind::Form));
		assert!(kinds.contains(&MacroKind::Head));
	}

	#[rstest]
	fn find_dsl_macros_ignores_non_reinhardt_macros() {
		// Arrange
		let source = r#"fn main() {
	let v = vec![1, 2, 3];
	println!("hello");
	assert_eq!(1, 1);
}"#;

		// Act
		let macros = find_dsl_macros(source).expect("parse source");

		// Assert
		assert_eq!(macros.len(), 0);
	}

	#[rstest]
	fn find_dsl_macros_ignores_macro_inside_string() {
		// Arrange
		let source = r#"fn main() {
	let s = "page!(|| { div {} })";
}"#;

		// Act
		let macros = find_dsl_macros(source).expect("parse source");

		// Assert
		assert_eq!(macros.len(), 0);
	}

	// -----------------------------------------------------------------------
	// format_dsl edge cases
	// -----------------------------------------------------------------------

	#[rstest]
	fn format_dsl_empty_closure_body() {
		// Arrange
		let input = "|| {}";

		// Act
		let formatted = format_dsl(MacroKind::Page, input).expect("format empty closure body");

		// Assert: should produce valid output without error
		assert!(
			formatted.contains("||"),
			"formatted output should contain closure syntax: {formatted}"
		);
	}

	#[rstest]
	fn format_dsl_nested_fragments() {
		// Arrange
		let input = r#"|| { div { span { a { "link" } } } }"#;

		// Act
		let formatted = format_dsl(MacroKind::Page, input).expect("format nested fragments");

		// Assert: each nesting level should produce deeper indentation
		assert!(
			formatted.contains("\tdiv {"),
			"outer fragment should be indented: {formatted}"
		);
		assert!(
			formatted.contains("\"link\""),
			"innermost text literal should be preserved: {formatted}"
		);
	}

	// -----------------------------------------------------------------------
	// apply_ignore_markers tests
	// -----------------------------------------------------------------------

	#[rstest]
	fn apply_ignore_markers_skips_macro_with_preceding_ignore_comment() {
		// Arrange
		let source = r#"fn main() {
	// reinhardt-fmt: ignore
	let view = page!(|| { div { "x" } });
}"#;
		let mut macros = find_dsl_macros(source).expect("parse source");

		// Act
		apply_ignore_markers(source, &mut macros);

		// Assert
		assert_eq!(macros.len(), 1);
		assert!(macros[0].should_skip);
	}

	#[rstest]
	fn apply_ignore_markers_does_not_skip_without_comment() {
		// Arrange
		let source = r#"fn main() {
	let view = page!(|| { div { "x" } });
}"#;
		let mut macros = find_dsl_macros(source).expect("parse source");

		// Act
		apply_ignore_markers(source, &mut macros);

		// Assert
		assert_eq!(macros.len(), 1);
		assert!(!macros[0].should_skip);
	}

	// -----------------------------------------------------------------------
	// SkipReason display
	// -----------------------------------------------------------------------

	#[rstest]
	fn skip_reason_display_file_wide_marker() {
		// Arrange
		let reason = SkipReason::FileWideMarker;

		// Act
		let display = format!("{reason}");

		// Assert
		assert_eq!(display, "file-wide ignore marker");
	}

	#[rstest]
	fn skip_reason_display_all_macros_ignored() {
		// Arrange
		let reason = SkipReason::AllMacrosIgnored;

		// Act
		let display = format!("{reason}");

		// Assert
		assert_eq!(display, "all macros ignored");
	}

	// -----------------------------------------------------------------------
	// macro_kind helper
	// -----------------------------------------------------------------------

	#[rstest]
	fn macro_kind_recognizes_page() {
		// Act
		let result = macro_kind("page! { }");

		// Assert
		assert_eq!(result, Some(MacroKind::Page));
	}

	#[rstest]
	fn macro_kind_recognizes_form() {
		// Act
		let result = macro_kind("form! { }");

		// Assert
		assert_eq!(result, Some(MacroKind::Form));
	}

	#[rstest]
	fn macro_kind_recognizes_head() {
		// Act
		let result = macro_kind("head! { }");

		// Assert
		assert_eq!(result, Some(MacroKind::Head));
	}

	#[rstest]
	fn macro_kind_recognizes_bare_style() {
		// Act
		let result = macro_kind("style! { .card { color: red; } }");

		// Assert
		assert_eq!(result, Some(MacroKind::Style));
	}

	#[rstest]
	#[case("crate::style! { .card { color: red; } }")]
	#[case("mystyle! { .card { color: red; } }")]
	#[case("style_alias! { .card { color: red; } }")]
	fn macro_kind_rejects_noncanonical_style_paths(#[case] source: &str) {
		// Arrange and Act
		let result = macro_kind(source);

		// Assert
		assert_eq!(result, None);
	}

	#[rstest]
	#[case(
		include_str!("../tests/fixtures/style/representative.input.rs"),
		include_str!("../tests/fixtures/style/representative.expected.rs")
	)]
	#[case(
		include_str!("../tests/fixtures/style/comments.input.rs"),
		include_str!("../tests/fixtures/style/comments.expected.rs")
	)]
	#[case(
		include_str!("../tests/fixtures/style/expressions.input.rs"),
		include_str!("../tests/fixtures/style/expressions.expected.rs")
	)]
	fn style_fixtures_format_exactly_and_idempotently(#[case] input: &str, #[case] expected: &str) {
		// Arrange
		let formatter = FormatEngine::new();

		// Act
		let formatted = formatter
			.format(input)
			.expect("style fixture should format");
		let second = formatter
			.format(&formatted.content)
			.expect("formatted fixture should reformat");

		// Assert
		assert_eq!(formatted.content, expected);
		assert_eq!(second.content, expected);
	}

	#[rstest]
	fn style_query_formats_a_definition_body() {
		// Arrange
		let input = "{ globals { border: Color; } .card { color: red; } }";

		// Act
		let formatted = format_dsl(MacroKind::Style, input).expect("style query should format");

		// Assert
		assert!(formatted.contains("border: Color;"));
	}

	#[rstest]
	fn macro_kind_returns_none_for_unknown() {
		// Act
		let result = macro_kind("vec! [1, 2, 3]");

		// Assert
		assert_eq!(result, None);
	}

	#[rstest]
	fn macro_kind_returns_none_for_prefix_without_bang() {
		// Act: "page" without "!" should not match
		let result = macro_kind("page { }");

		// Assert
		assert_eq!(result, None);
	}

	// -----------------------------------------------------------------------
	// base_indent helper
	// -----------------------------------------------------------------------

	#[rstest]
	fn base_indent_zero_for_first_line() {
		// Arrange
		let content = "page!(|| { div {} })";

		// Act
		let indent = base_indent(content, 0);

		// Assert
		assert_eq!(indent, 0);
	}

	#[rstest]
	fn base_indent_counts_tabs() {
		// Arrange
		let content = "fn main() {\n\t\tlet view = page!(|| {});\n}";
		let offset = content.find("page").expect("find page");

		// Act
		let indent = base_indent(content, offset);

		// Assert: two tabs before "let"
		assert_eq!(indent, 2);
	}

	// -----------------------------------------------------------------------
	// indent_relative helper
	// -----------------------------------------------------------------------

	#[rstest]
	fn indent_relative_no_indent() {
		// Arrange
		let input = "line1\nline2";

		// Act
		let result = indent_relative(input, 0);

		// Assert
		assert_eq!(result, "line1\nline2");
	}

	#[rstest]
	fn indent_relative_adds_tabs_to_subsequent_lines() {
		// Arrange
		let input = "line1\nline2\nline3";

		// Act
		let result = indent_relative(input, 1);

		// Assert: first line unchanged, subsequent lines get one tab prefix
		assert_eq!(result, "line1\n\tline2\n\tline3");
	}

	#[rstest]
	fn indent_relative_preserves_empty_lines() {
		// Arrange
		let input = "line1\n\nline3";

		// Act
		let result = indent_relative(input, 1);

		// Assert: empty line should remain empty (no indent added)
		assert_eq!(result, "line1\n\n\tline3");
	}

	// -----------------------------------------------------------------------
	// FormatResult field validation
	// -----------------------------------------------------------------------

	#[rstest]
	fn format_result_contains_dsl_macro_true_when_macro_present() {
		// Arrange
		let formatter = FormatEngine::new();
		let source = r#"fn main() { let v = page!(|| { div { "x" } }); }"#;

		// Act
		let result = formatter.format(source).expect("format source");

		// Assert
		assert!(result.contains_dsl_macro);
	}

	#[rstest]
	fn format_result_contains_dsl_macro_false_when_no_macro() {
		// Arrange
		let formatter = FormatEngine::new();
		let source = "fn main() { println!(\"hello\"); }";

		// Act
		let result = formatter.format(source).expect("format source");

		// Assert
		assert!(!result.contains_dsl_macro);
	}

	#[rstest]
	fn single_variable_block_stays_inline() {
		// Arrange / Act
		let formatted = format_dsl(
			MacroKind::Page,
			r#"|| { span { disabled: true, { text } } }"#,
		)
		.expect("format DSL");

		// Assert
		assert!(
			formatted.contains("{ text }"),
			"single-variable block should stay inline: {formatted}"
		);
	}

	#[rstest]
	fn space_after_colon_before_paren() {
		// Arrange / Act
		let formatted = format_dsl(
			MacroKind::Page,
			r#"|| { span { tabindex: (-1_i32).to_string(), } }"#,
		)
		.expect("format DSL");

		// Assert
		assert!(
			formatted.contains("tabindex: (-1_i32)"),
			"space after colon and no space inside unary minus: {formatted}"
		);
	}

	#[rstest]
	fn form_and_head_unary_minus_spacing_is_preserved() {
		for (kind, input) in [
			(MacroKind::Form, r#"{ tabindex: (-1_i32), }"#),
			(MacroKind::Head, r#"|| { meta { tabindex: (-1_i32), } }"#),
		] {
			// Arrange / Act
			let formatted = format_dsl(kind, input).expect("format DSL");

			// Assert
			assert!(
				formatted.contains("tabindex: (-1_i32)"),
				"{kind:?} should preserve unary-minus spacing without page rustfmt islands: {formatted}"
			);
		}
	}

	#[rstest]
	fn page_rustfmt_island_defaults_to_2024_edition_with_config_path() {
		// Arrange
		let dir = tempfile::tempdir().expect("create tempdir");
		let config_path = dir.path().join("rustfmt.toml");
		std::fs::write(&config_path, "max_width = 100\n").expect("write rustfmt config");
		let options = RustfmtOptions {
			config_path: Some(config_path),
			..RustfmtOptions::default()
		};

		// Act
		let formatted = format_dsl_with_options(
			MacroKind::Page,
			r#"|| { div { value: async { foo+bar }, } }"#,
			&options,
		)
		.expect("format DSL");

		// Assert
		assert!(
			formatted.contains("value: async { foo + bar },"),
			"page Rust islands should use Rust 2024 even when config_path is supplied: {formatted}"
		);
	}

	#[rstest]
	fn page_rustfmt_island_formats_attribute_expression() {
		// Arrange / Act
		let formatted = format_dsl(
			MacroKind::Page,
			r#"|| { span { title: format!("{}", foo+bar), } }"#,
		)
		.expect("format DSL");

		// Assert
		assert!(
			formatted.contains(r#"title: format!("{}", foo + bar),"#),
			"attribute expression should be formatted inside page Rust island: {formatted}"
		);
	}

	#[rstest]
	fn page_rustfmt_island_formats_event_closure_body() {
		// Arrange / Act
		let formatted = format_dsl(
			MacroKind::Page,
			r#"|| { button { @click: |_| { match status { 0=>set_pending(), _=>set_done(), } }, } }"#,
		)
		.expect("format DSL");

		// Assert
		assert!(
			formatted.contains("0 => set_pending(),"),
			"match arm spacing should be formatted inside event closure body: {formatted}"
		);
		assert!(
			formatted.contains("_ => set_done(),"),
			"fallback match arm spacing should be formatted inside event closure body: {formatted}"
		);
	}

	#[rstest]
	fn page_rustfmt_island_preserves_match_arm_space_after_pattern_call() {
		// Arrange
		let formatter = FormatEngine::new();
		let source = r#"fn render() {
	let view = page!(|| {
		{
			match resource {
				ResourceState::Error(error) => {
					message
				}
				_ => {}
			}
		}
	});
}"#;

		// Act
		let formatted = formatter.format(source).expect("format source").content;

		// Assert
		assert!(
			formatted.contains("ResourceState::Error(error) => {"),
			"match-arm arrow should retain a leading space after a pattern call: {formatted}"
		);
	}

	#[rstest]
	fn page_rustfmt_island_formats_closure_with_multiple_parameters() {
		// Arrange / Act
		let formatted = format_dsl(
			MacroKind::Page,
			r#"|| { button { on_pair: |left, right| left+right, "Save" } }"#,
		)
		.expect("format DSL");

		// Assert
		assert!(
			formatted.contains("on_pair: |left, right| left + right,"),
			"closure parameter comma should stay inside the Rust island: {formatted}"
		);
	}

	#[rstest]
	fn page_rustfmt_island_formats_turbofish_with_multiple_arguments() {
		// Arrange / Act
		let formatted = format_dsl(
			MacroKind::Page,
			r#"|| { div { value: make_pair::<Left, Right>(), "x" } }"#,
		)
		.expect("format DSL");

		// Assert
		assert!(
			formatted.contains("value: make_pair::<Left, Right>(),"),
			"turbofish comma should stay inside the Rust island: {formatted}"
		);
	}

	#[rstest]
	fn page_rustfmt_island_formats_interpolation_expression() {
		// Arrange / Act
		let formatted =
			format_dsl(MacroKind::Page, r#"|| { span { { count+1 } } }"#).expect("format DSL");

		// Assert
		assert!(
			formatted.contains("{ count + 1 }"),
			"interpolation expression should be formatted inside page Rust island: {formatted}"
		);
	}

	#[rstest]
	fn page_rustfmt_island_formats_interpolation_paths() {
		// Arrange / Act
		let formatted = format_dsl(MacroKind::Page, r#"|| { span { { foo::bar(1+2) } } }"#)
			.expect("format DSL");

		// Assert
		assert!(
			formatted.contains("{ foo::bar(1 + 2) }"),
			"Rust path separators inside interpolation should not become attributes: {formatted}"
		);
	}

	#[rstest]
	fn page_rustfmt_island_preserves_top_level_statement_interpolation() {
		// Arrange / Act
		let formatted = format_dsl(
			MacroKind::Page,
			r#"|| { div { { hooks::use_effect({ let count = count.clone(); move || { let _ = count.get(); None::<fn() > } }, (count.clone(), ), ); "x" } } }"#,
		)
		.expect("format DSL");

		// Assert
		assert!(
			formatted.contains("hooks::use_effect"),
			"Rust paths in statement interpolation should be preserved: {formatted}"
		);
		assert!(
			formatted.contains(r#"; "x""#),
			"top-level statement interpolation should not lose following expressions: {formatted}"
		);
		assert!(
			!formatted.contains("hooks: :use_effect"),
			"Rust path separators should not be formatted as DSL attributes: {formatted}"
		);
	}

	#[rstest]
	fn page_rustfmt_island_preserves_string_contents() {
		// Arrange / Act
		let formatted = format_dsl(
			MacroKind::Page,
			r#"|| { span { title: "value is (- 1)", { "match x { 0=>a }" } } }"#,
		)
		.expect("format DSL");

		// Assert
		assert!(
			formatted.contains(r#""value is (- 1)""#),
			"string literal contents should remain unchanged: {formatted}"
		);
		assert!(
			formatted.contains(r#""match x { 0=>a }""#),
			"text string contents should remain unchanged: {formatted}"
		);
	}

	#[rstest]
	fn page_rustfmt_island_falls_back_for_invalid_rust() {
		// Arrange / Act
		let formatted = format_dsl(MacroKind::Page, r#"|| { span { title: value., "x" } }"#)
			.expect("format DSL");

		// Assert
		assert!(
			formatted.contains("title: value.,"),
			"invalid Rust island should be preserved when rustfmt cannot format it: {formatted}"
		);
	}

	#[rstest]
	fn page_rustfmt_island_preserves_comment_islands() {
		// Arrange / Act
		let formatted = format_dsl(
			MacroKind::Page,
			r#"|| { span { title: /* keep */ foo+bar, } }"#,
		)
		.expect("format DSL");

		// Assert
		assert!(
			formatted.contains("title: /* keep */ foo+bar,"),
			"comment-bearing Rust islands should be preserved: {formatted}"
		);
	}

	#[rstest]
	fn page_rustfmt_island_preserves_line_comment_islands_idempotently() {
		// Arrange
		let input = "|| { span { title: // keep leading\nfoo+bar, other: value+1, } }";

		// Act
		let formatted = format_dsl(MacroKind::Page, input).expect("format DSL");
		let second = format_dsl(MacroKind::Page, &formatted).expect("format DSL again");

		// Assert
		assert_eq!(
			second, formatted,
			"line-comment Rust islands should be formatter-idempotent"
		);
		assert!(
			formatted.contains("title: // keep leading\nfoo+bar,"),
			"line-comment Rust islands should be preserved: {formatted}"
		);
		assert!(
			formatted.contains("other: value + 1,"),
			"safe sibling Rust islands should still be formatted: {formatted}"
		);
	}

	#[rstest]
	fn page_rustfmt_island_uses_configured_rustfmt_options() {
		// Arrange
		let options = RustfmtOptions {
			config: Some("max_width=40".to_string()),
			..RustfmtOptions::default()
		};

		// Act
		let formatted = format_dsl_with_options(
			MacroKind::Page,
			r#"|| { span { title: very_long_function_name(alpha_value, beta_value, gamma_value, delta_value), } }"#,
			&options,
		)
		.expect("format DSL");

		// Assert
		assert!(
			formatted.contains("\n\t\t\talpha_value,"),
			"configured rustfmt max_width should format page Rust islands: {formatted}"
		);
	}

	#[rstest]
	fn empty_closure_body_stays_inline() {
		// Arrange / Act
		let formatted = format_dsl(
			MacroKind::Page,
			r#"|| { div { @click: |_| { }, @dblclick: |_| { }, } }"#,
		)
		.expect("format DSL");

		// Assert
		assert!(
			formatted.contains("|_| {},"),
			"empty closure body should stay inline: {formatted}"
		);
	}

	#[rstest]
	fn else_if_stays_on_same_line_as_closing_brace() {
		// Arrange / Act
		let formatted = format_dsl(
			MacroKind::Page,
			r#"|| { if status == 0 { span { "Pending" } } else if status == 1 { span { "Processing" } } else { span { "Done" } } }"#,
		)
		.expect("format DSL");

		// Assert
		assert!(
			formatted.contains("} else if"),
			"else if should be on same line as closing brace: {formatted}"
		);
		assert!(
			formatted.contains("} else {"),
			"else should be on same line as closing brace: {formatted}"
		);
	}

	#[rstest]
	fn control_flow_struct_patterns_keep_real_body() {
		// Arrange / Act
		let formatted = format_dsl(
			MacroKind::Page,
			r#"|| { if let Foo { field } = value { div { "x" } } for Foo { field } in rows { span { { field } } } }"#,
		)
		.expect("format DSL");

		// Assert
		assert!(
			formatted.contains(r#"if let Foo { field } = value {"#),
			"if-let struct pattern should stay in the control-flow head: {formatted}"
		);
		assert!(
			formatted.contains(r#"for Foo { field } in rows {"#),
			"for-loop struct pattern should stay in the control-flow head: {formatted}"
		);
	}

	#[rstest]
	fn empty_element_block_stays_inline() {
		// Arrange / Act
		let formatted = format_dsl(MacroKind::Page, r#"|| { div {} div {} }"#).expect("format DSL");

		// Assert
		assert!(
			formatted.contains("div {}"),
			"empty element block should stay inline: {formatted}"
		);
		assert!(
			!formatted.contains("div {\n"),
			"empty element block should not expand: {formatted}"
		);
	}

	#[rstest]
	fn style_body_rustfmt_protection_round_trips_multiple_bodies() {
		// Arrange
		let source = r###"fn styles() {
	#[style_def]
	static FIRST: FirstStyles = style! {
		.card { transform: unchecked_fn!("fn({ raw })"); }
	};
	// reinhardt-fmt: ignore
	#[style_def]
	static SECOND: SecondStyles = style! { .item { color: red; } };
}"###;

		// Act
		let (protected_source, bodies) = protect_style_bodies(source).expect("protect bodies");
		let restored = restore_style_bodies(&protected_source, &bodies).expect("restore bodies");

		// Assert
		assert_eq!(restored, source);
		assert_eq!(
			protected_source.matches("__reinhardt_style_body_").count(),
			2
		);
	}

	#[rstest]
	fn style_body_rustfmt_protection_avoids_existing_marker_prefix() {
		// Arrange
		let source = "const __reinhardt_style_body_0: () = ();\n\
#[style_def]\n\
static STYLES: Styles = style! { .card { color: red; } };\n";

		// Act
		let (protected_source, bodies) = protect_style_bodies(source).expect("protect bodies");
		let restored = restore_style_bodies(&protected_source, &bodies).expect("restore bodies");

		// Assert
		assert!(protected_source.contains("__reinhardt_style_body_1"));
		assert_eq!(restored, source);
	}

	#[rstest]
	fn style_body_rustfmt_protection_handles_no_styles() {
		// Arrange
		let source = "fn main() {}\n";

		// Act
		let (protected_source, bodies) = protect_style_bodies(source).expect("protect bodies");
		let restored = restore_style_bodies(&protected_source, &bodies).expect("restore bodies");

		// Assert
		assert_eq!(protected_source, source);
		assert_eq!(restored, source);
	}
}
