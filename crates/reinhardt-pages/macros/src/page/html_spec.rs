//! HTML element specifications for validation.
//!
//! This module defines HTML element specifications based on the WHATWG HTML Standard.
//! It provides validation rules for element-specific attributes, required attributes,
//! and content models.
//!
//! # Priority
//!
//! Elements are categorized by priority:
//! - **High Priority**: Commonly used elements with strict rules (img, a, input, button, form)
//! - **Medium Priority**: Basic structural elements (div, span, p, headings)
//! - **Low Priority**: Lists, tables, and other less commonly validated elements

use syn::Result;

use reinhardt_manouche::core::TypedPageElement;

/// HTML element specification.
#[derive(Debug, Clone)]
pub(crate) struct ElementSpec {
	/// Element tag name
	pub tag: &'static str,
	/// Required attributes (must be present)
	pub required_attrs: &'static [AttrSpec],
	/// Allowed attributes (None = all attributes allowed)
	pub allowed_attrs: Option<&'static [&'static str]>,
	/// Whether this is a void element (no children allowed)
	#[allow(dead_code)] // Reserved for Phase 3
	pub is_void: bool,
	/// Whether this is an interactive element (cannot nest)
	#[allow(dead_code)] // Reserved for Phase 3
	pub is_interactive: bool,
	/// Content model constraints
	pub content_model: Option<ContentModel>,
}

/// Attribute specification.
#[derive(Debug, Clone)]
pub(crate) struct AttrSpec {
	/// Attribute name
	pub name: &'static str,
	/// Expected type of attribute value
	#[allow(dead_code)] // Reserved for Phase 3
	pub expected_type: AttrType,
	/// Whether this attribute is required
	pub required: bool,
}

/// Expected type for attribute values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttrType {
	/// String value (any string)
	String,
	/// URL value (should be string literal for static validation)
	Url,
	/// Boolean value (true/false)
	#[allow(dead_code)] // Reserved for Phase 3
	Boolean,
	/// Numeric value (integer or float)
	#[allow(dead_code)] // Reserved for Phase 3
	Number,
	/// Any type (no validation)
	#[allow(dead_code)] // Reserved for Phase 3
	Any,
}

/// Content model constraints.
#[derive(Debug, Clone)]
pub(crate) enum ContentModel {
	/// Only specific child tags are allowed
	OnlyTags(&'static [&'static str]),
	/// Only text content (no elements)
	TextOnly,
	/// No children allowed (void elements)
	Empty,
}

/// Enumerated attribute specification.
///
/// Defines valid values for attributes that accept a limited set of string values.
#[derive(Debug, Clone, Copy)]
pub(crate) struct EnumAttrSpec {
	/// Attribute name
	pub name: &'static str,
	/// List of valid values for this attribute
	pub valid_values: &'static [&'static str],
}

/// Element-specific enumerated attributes list.
///
/// Maps an element tag to its enumerated attributes.
#[derive(Debug, Clone)]
pub(crate) struct ElementEnumAttrs {
	/// Element tag name
	/// Allow dead_code: Field reserved for future debugging/validation features
	#[allow(dead_code)]
	pub tag: &'static str,
	/// Enumerated attributes for this element
	pub attrs: &'static [EnumAttrSpec],
}

/// Global HTML attributes (allowed on all elements).
///
/// Based on: <https://developer.mozilla.org/en-US/docs/Web/HTML/Global_attributes>
pub(crate) static GLOBAL_ATTRS: &[&str] = &[
	// Core attributes
	"id",
	"class",
	"style",
	"title",
	"lang",
	"dir",
	// Interaction attributes
	"tabindex",
	"accesskey",
	"contenteditable",
	"draggable",
	"hidden",
	"spellcheck",
	"translate",
	// ARIA role attribute
	"role",
	// Data attributes (data-*)
	// ARIA attributes (aria-*)
	// Note: data-* and aria-* are validated with pattern matching
];

//
// High Priority Element Specifications
//

/// Specification for `<img>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/img>
pub(super) static IMG_SPEC: ElementSpec = ElementSpec {
	tag: "img",
	required_attrs: &[
		AttrSpec {
			name: "src",
			expected_type: AttrType::Url,
			required: true,
		},
		AttrSpec {
			name: "alt",
			expected_type: AttrType::String,
			required: true,
		},
	],
	allowed_attrs: Some(&[
		// Required
		"src",
		"alt",
		// Optional
		"width",
		"height",
		"loading",
		"decoding",
		"srcset",
		"sizes",
		"crossorigin",
		"usemap",
		"ismap",
		"referrerpolicy",
	]),
	is_void: true,
	is_interactive: false,
	content_model: Some(ContentModel::Empty),
};

/// Specification for `<a>` (anchor) element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/a>
pub(super) static A_SPEC: ElementSpec = ElementSpec {
	tag: "a",
	required_attrs: &[],
	allowed_attrs: Some(&[
		"href",
		"target",
		"download",
		"ping",
		"rel",
		"hreflang",
		"type",
		"referrerpolicy",
	]),
	is_void: false,
	is_interactive: true,
	content_model: None, // Flow content (but no nested interactive elements)
};

/// Specification for `<button>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/button>
pub(super) static BUTTON_SPEC: ElementSpec = ElementSpec {
	tag: "button",
	required_attrs: &[],
	allowed_attrs: Some(&[
		"type",
		"name",
		"value",
		"disabled",
		"form",
		"formaction",
		"formenctype",
		"formmethod",
		"formnovalidate",
		"formtarget",
		"autofocus",
		"popovertarget",
		"popovertargetaction",
	]),
	is_void: false,
	is_interactive: true,
	content_model: None, // Phrasing content (but no nested interactive elements)
};

/// Specification for `<input>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/input>
pub(super) static INPUT_SPEC: ElementSpec = ElementSpec {
	tag: "input",
	required_attrs: &[],
	allowed_attrs: Some(&[
		"type",
		"name",
		"value",
		"placeholder",
		"disabled",
		"readonly",
		"required",
		"autofocus",
		"autocomplete",
		"min",
		"max",
		"step",
		"pattern",
		"maxlength",
		"minlength",
		"size",
		"multiple",
		"accept",
		"checked",
		"form",
		"formaction",
		"formenctype",
		"formmethod",
		"formnovalidate",
		"formtarget",
		"list",
		"src",
		"alt",
		"width",
		"height",
	]),
	is_void: true,
	is_interactive: true,
	content_model: Some(ContentModel::Empty),
};

/// Specification for `<form>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/form>
pub(super) static FORM_SPEC: ElementSpec = ElementSpec {
	tag: "form",
	required_attrs: &[],
	allowed_attrs: Some(&[
		"action",
		"method",
		"enctype",
		"name",
		"target",
		"novalidate",
		"autocomplete",
		"accept-charset",
		"rel",
	]),
	is_void: false,
	is_interactive: false,
	content_model: None, // Flow content (but no nested form elements)
};

/// Specification for `<label>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/label>
pub(super) static LABEL_SPEC: ElementSpec = ElementSpec {
	tag: "label",
	required_attrs: &[],
	allowed_attrs: Some(&["for", "form"]),
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<select>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/select>
pub(super) static SELECT_SPEC: ElementSpec = ElementSpec {
	tag: "select",
	required_attrs: &[],
	allowed_attrs: Some(&[
		"name",
		"disabled",
		"required",
		"autofocus",
		"form",
		"multiple",
		"size",
	]),
	is_void: false,
	is_interactive: true,
	content_model: Some(ContentModel::OnlyTags(&["option", "optgroup"])),
};

/// Specification for `<textarea>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/textarea>
pub(super) static TEXTAREA_SPEC: ElementSpec = ElementSpec {
	tag: "textarea",
	required_attrs: &[],
	allowed_attrs: Some(&[
		"name",
		"rows",
		"cols",
		"disabled",
		"readonly",
		"required",
		"autofocus",
		"autocomplete",
		"maxlength",
		"minlength",
		"placeholder",
		"wrap",
		"form",
		"spellcheck",
	]),
	is_void: false,
	is_interactive: true,
	content_model: Some(ContentModel::TextOnly),
};

/// Specification for `<option>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/option>
pub(super) static OPTION_SPEC: ElementSpec = ElementSpec {
	tag: "option",
	required_attrs: &[],
	allowed_attrs: Some(&["value", "label", "disabled", "selected"]),
	is_void: false,
	is_interactive: false,
	content_model: Some(ContentModel::TextOnly),
};

//
// Medium Priority Element Specifications
//

/// Specification for `<div>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/div>
pub(super) static DIV_SPEC: ElementSpec = ElementSpec {
	tag: "div",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Flow content
};

/// Specification for `<span>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/span>
pub(super) static SPAN_SPEC: ElementSpec = ElementSpec {
	tag: "span",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<p>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/p>
pub(super) static P_SPEC: ElementSpec = ElementSpec {
	tag: "p",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<h1>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/h1>
pub(super) static H1_SPEC: ElementSpec = ElementSpec {
	tag: "h1",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<h2>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/h2>
pub(super) static H2_SPEC: ElementSpec = ElementSpec {
	tag: "h2",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<h3>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/h3>
pub(super) static H3_SPEC: ElementSpec = ElementSpec {
	tag: "h3",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<h4>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/h4>
pub(super) static H4_SPEC: ElementSpec = ElementSpec {
	tag: "h4",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<h5>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/h5>
pub(super) static H5_SPEC: ElementSpec = ElementSpec {
	tag: "h5",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<h6>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/h6>
pub(super) static H6_SPEC: ElementSpec = ElementSpec {
	tag: "h6",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<header>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/header>
pub(super) static HEADER_SPEC: ElementSpec = ElementSpec {
	tag: "header",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Flow content (but no nested header/footer)
};

/// Specification for `<footer>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/footer>
pub(super) static FOOTER_SPEC: ElementSpec = ElementSpec {
	tag: "footer",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Flow content (but no nested header/footer)
};

/// Specification for `<main>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/main>
pub(super) static MAIN_SPEC: ElementSpec = ElementSpec {
	tag: "main",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Flow content
};

/// Specification for `<nav>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/nav>
pub(super) static NAV_SPEC: ElementSpec = ElementSpec {
	tag: "nav",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Flow content
};

/// Specification for `<section>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/section>
pub(super) static SECTION_SPEC: ElementSpec = ElementSpec {
	tag: "section",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Flow content
};

/// Specification for `<article>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/article>
pub(super) static ARTICLE_SPEC: ElementSpec = ElementSpec {
	tag: "article",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Flow content
};

//
// Text-Level (Phrasing) Element Specifications
//

/// Specification for `<em>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/em>
pub(super) static EM_SPEC: ElementSpec = ElementSpec {
	tag: "em",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<strong>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/strong>
pub(super) static STRONG_SPEC: ElementSpec = ElementSpec {
	tag: "strong",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<small>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/small>
pub(super) static SMALL_SPEC: ElementSpec = ElementSpec {
	tag: "small",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<code>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/code>
pub(super) static CODE_SPEC: ElementSpec = ElementSpec {
	tag: "code",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<kbd>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/kbd>
pub(super) static KBD_SPEC: ElementSpec = ElementSpec {
	tag: "kbd",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<samp>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/samp>
pub(super) static SAMP_SPEC: ElementSpec = ElementSpec {
	tag: "samp",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<var>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/var>
pub(super) static VAR_SPEC: ElementSpec = ElementSpec {
	tag: "var",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<i>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/i>
pub(super) static I_SPEC: ElementSpec = ElementSpec {
	tag: "i",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<b>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/b>
pub(super) static B_SPEC: ElementSpec = ElementSpec {
	tag: "b",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<u>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/u>
pub(super) static U_SPEC: ElementSpec = ElementSpec {
	tag: "u",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<mark>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/mark>
pub(super) static MARK_SPEC: ElementSpec = ElementSpec {
	tag: "mark",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<s>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/s>
pub(super) static S_SPEC: ElementSpec = ElementSpec {
	tag: "s",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<sub>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/sub>
pub(super) static SUB_SPEC: ElementSpec = ElementSpec {
	tag: "sub",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<sup>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/sup>
pub(super) static SUP_SPEC: ElementSpec = ElementSpec {
	tag: "sup",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Phrasing content
};

/// Specification for `<br>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/br>
pub(super) static BR_SPEC: ElementSpec = ElementSpec {
	tag: "br",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: true,
	is_interactive: false,
	content_model: Some(ContentModel::Empty),
};

/// Specification for `<wbr>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/wbr>
pub(super) static WBR_SPEC: ElementSpec = ElementSpec {
	tag: "wbr",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: true,
	is_interactive: false,
	content_model: Some(ContentModel::Empty),
};

//
// List Element Specifications
//

/// Specification for `<ul>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/ul>
pub(super) static UL_SPEC: ElementSpec = ElementSpec {
	tag: "ul",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: Some(ContentModel::OnlyTags(&["li"])),
};

/// Specification for `<ol>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/ol>
pub(super) static OL_SPEC: ElementSpec = ElementSpec {
	tag: "ol",
	required_attrs: &[],
	allowed_attrs: Some(&["reversed", "start", "type"]),
	is_void: false,
	is_interactive: false,
	content_model: Some(ContentModel::OnlyTags(&["li"])),
};

/// Specification for `<li>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/li>
pub(super) static LI_SPEC: ElementSpec = ElementSpec {
	tag: "li",
	required_attrs: &[],
	allowed_attrs: Some(&["value"]),
	is_void: false,
	is_interactive: false,
	content_model: None, // Flow content
};

/// Specification for `<dl>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/dl>
pub(super) static DL_SPEC: ElementSpec = ElementSpec {
	tag: "dl",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: Some(ContentModel::OnlyTags(&["dt", "dd", "div"])),
};

/// Specification for `<dt>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/dt>
pub(super) static DT_SPEC: ElementSpec = ElementSpec {
	tag: "dt",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Flow content (but no header/footer/sectioning/heading)
};

/// Specification for `<dd>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/dd>
pub(super) static DD_SPEC: ElementSpec = ElementSpec {
	tag: "dd",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Flow content
};

//
// Table Element Specifications
//

/// Specification for `<table>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/table>
pub(super) static TABLE_SPEC: ElementSpec = ElementSpec {
	tag: "table",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: Some(ContentModel::OnlyTags(&[
		"caption", "colgroup", "thead", "tbody", "tfoot", "tr",
	])),
};

/// Specification for `<thead>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/thead>
pub(super) static THEAD_SPEC: ElementSpec = ElementSpec {
	tag: "thead",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: Some(ContentModel::OnlyTags(&["tr"])),
};

/// Specification for `<tbody>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/tbody>
pub(super) static TBODY_SPEC: ElementSpec = ElementSpec {
	tag: "tbody",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: Some(ContentModel::OnlyTags(&["tr"])),
};

/// Specification for `<tfoot>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/tfoot>
pub(super) static TFOOT_SPEC: ElementSpec = ElementSpec {
	tag: "tfoot",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: Some(ContentModel::OnlyTags(&["tr"])),
};

/// Specification for `<tr>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/tr>
pub(super) static TR_SPEC: ElementSpec = ElementSpec {
	tag: "tr",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: Some(ContentModel::OnlyTags(&["th", "td"])),
};

/// Specification for `<th>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/th>
pub(super) static TH_SPEC: ElementSpec = ElementSpec {
	tag: "th",
	required_attrs: &[],
	allowed_attrs: Some(&["colspan", "rowspan", "headers", "scope", "abbr"]),
	is_void: false,
	is_interactive: false,
	content_model: None, // Flow content (but no header/footer/sectioning/heading)
};

/// Specification for `<td>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/td>
pub(super) static TD_SPEC: ElementSpec = ElementSpec {
	tag: "td",
	required_attrs: &[],
	allowed_attrs: Some(&["colspan", "rowspan", "headers"]),
	is_void: false,
	is_interactive: false,
	content_model: None, // Flow content
};

/// Specification for `<caption>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/caption>
pub(super) static CAPTION_SPEC: ElementSpec = ElementSpec {
	tag: "caption",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed
	is_void: false,
	is_interactive: false,
	content_model: None, // Flow content (but no table elements)
};

/// Specification for `<colgroup>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/colgroup>
pub(super) static COLGROUP_SPEC: ElementSpec = ElementSpec {
	tag: "colgroup",
	required_attrs: &[],
	allowed_attrs: Some(&["span"]),
	is_void: false,
	is_interactive: false,
	content_model: Some(ContentModel::OnlyTags(&["col"])),
};

/// Specification for `<col>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/col>
pub(super) static COL_SPEC: ElementSpec = ElementSpec {
	tag: "col",
	required_attrs: &[],
	allowed_attrs: Some(&["span"]),
	is_void: true,
	is_interactive: false,
	content_model: Some(ContentModel::Empty),
};

// ============================================================================
// Embedded Content Elements
// ============================================================================

/// Specification for `<iframe>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/iframe>
pub(super) static IFRAME_SPEC: ElementSpec = ElementSpec {
	tag: "iframe",
	required_attrs: &[],
	allowed_attrs: Some(&[
		"src",
		"srcdoc",
		"name",
		"sandbox",
		"allow",
		"allowfullscreen",
		"width",
		"height",
		"referrerpolicy",
		"loading",
	]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<video>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/video>
pub(super) static VIDEO_SPEC: ElementSpec = ElementSpec {
	tag: "video",
	required_attrs: &[],
	allowed_attrs: Some(&[
		"src",
		"poster",
		"preload",
		"autoplay",
		"playsinline",
		"loop",
		"muted",
		"controls",
		"width",
		"height",
		"crossorigin",
	]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<audio>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/audio>
pub(super) static AUDIO_SPEC: ElementSpec = ElementSpec {
	tag: "audio",
	required_attrs: &[],
	allowed_attrs: Some(&[
		"src",
		"preload",
		"autoplay",
		"loop",
		"muted",
		"controls",
		"crossorigin",
	]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<source>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/source>
pub(super) static SOURCE_SPEC: ElementSpec = ElementSpec {
	tag: "source",
	required_attrs: &[],
	allowed_attrs: Some(&["src", "type", "srcset", "sizes", "media", "width", "height"]),
	is_void: true,
	is_interactive: false,
	content_model: Some(ContentModel::Empty),
};

/// Specification for `<track>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/track>
pub(super) static TRACK_SPEC: ElementSpec = ElementSpec {
	tag: "track",
	required_attrs: &[AttrSpec {
		name: "src",
		expected_type: AttrType::String,
		required: true,
	}],
	// Fixes #851: include required attribute "src" in allowed_attrs
	allowed_attrs: Some(&["src", "kind", "label", "srclang", "default"]),
	is_void: true,
	is_interactive: false,
	content_model: Some(ContentModel::Empty),
};

/// Specification for `<canvas>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/canvas>
pub(super) static CANVAS_SPEC: ElementSpec = ElementSpec {
	tag: "canvas",
	required_attrs: &[],
	allowed_attrs: Some(&["width", "height"]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<script>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/script>
pub(super) static SCRIPT_SPEC: ElementSpec = ElementSpec {
	tag: "script",
	required_attrs: &[],
	allowed_attrs: Some(&[
		"src",
		"type",
		"nomodule",
		"async",
		"defer",
		"crossorigin",
		"integrity",
		"referrerpolicy",
	]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<noscript>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/noscript>
pub(super) static NOSCRIPT_SPEC: ElementSpec = ElementSpec {
	tag: "noscript",
	required_attrs: &[],
	allowed_attrs: Some(&[]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<embed>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/embed>
pub(super) static EMBED_SPEC: ElementSpec = ElementSpec {
	tag: "embed",
	required_attrs: &[],
	allowed_attrs: Some(&["src", "type", "width", "height"]),
	is_void: true,
	is_interactive: false,
	content_model: Some(ContentModel::Empty),
};

/// Specification for `<object>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/object>
pub(super) static OBJECT_SPEC: ElementSpec = ElementSpec {
	tag: "object",
	required_attrs: &[],
	allowed_attrs: Some(&["data", "type", "name", "usemap", "form", "width", "height"]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<param>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/param>
pub(super) static PARAM_SPEC: ElementSpec = ElementSpec {
	tag: "param",
	required_attrs: &[AttrSpec {
		name: "name",
		expected_type: AttrType::String,
		required: true,
	}],
	// Fixes #851: include required attribute "name" in allowed_attrs
	allowed_attrs: Some(&["name", "value"]),
	is_void: true,
	is_interactive: false,
	content_model: Some(ContentModel::Empty),
};

/// Specification for `<picture>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/picture>
pub(super) static PICTURE_SPEC: ElementSpec = ElementSpec {
	tag: "picture",
	required_attrs: &[],
	allowed_attrs: Some(&[]),
	is_void: false,
	is_interactive: false,
	content_model: Some(ContentModel::OnlyTags(&["source", "img"])),
};

// ============================================================================
// Other Important Elements
// ============================================================================

/// Specification for `<hr>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/hr>
pub(super) static HR_SPEC: ElementSpec = ElementSpec {
	tag: "hr",
	required_attrs: &[],
	allowed_attrs: Some(&[]),
	is_void: true,
	is_interactive: false,
	content_model: Some(ContentModel::Empty),
};

/// Specification for `<pre>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/pre>
pub(super) static PRE_SPEC: ElementSpec = ElementSpec {
	tag: "pre",
	required_attrs: &[],
	allowed_attrs: Some(&[]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<blockquote>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/blockquote>
pub(super) static BLOCKQUOTE_SPEC: ElementSpec = ElementSpec {
	tag: "blockquote",
	required_attrs: &[],
	allowed_attrs: Some(&["cite"]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<q>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/q>
pub(super) static Q_SPEC: ElementSpec = ElementSpec {
	tag: "q",
	required_attrs: &[],
	allowed_attrs: Some(&["cite"]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<cite>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/cite>
pub(super) static CITE_SPEC: ElementSpec = ElementSpec {
	tag: "cite",
	required_attrs: &[],
	allowed_attrs: Some(&[]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<abbr>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/abbr>
pub(super) static ABBR_SPEC: ElementSpec = ElementSpec {
	tag: "abbr",
	required_attrs: &[],
	allowed_attrs: Some(&[]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<time>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/time>
pub(super) static TIME_SPEC: ElementSpec = ElementSpec {
	tag: "time",
	required_attrs: &[],
	allowed_attrs: Some(&["datetime"]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<data>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/data>
pub(super) static DATA_SPEC: ElementSpec = ElementSpec {
	tag: "data",
	required_attrs: &[AttrSpec {
		name: "value",
		expected_type: AttrType::String,
		required: true,
	}],
	// Fixes #851: include required attribute "value" in allowed_attrs
	allowed_attrs: Some(&["value"]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<dfn>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/dfn>
pub(super) static DFN_SPEC: ElementSpec = ElementSpec {
	tag: "dfn",
	required_attrs: &[],
	allowed_attrs: Some(&[]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<ins>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/ins>
pub(super) static INS_SPEC: ElementSpec = ElementSpec {
	tag: "ins",
	required_attrs: &[],
	allowed_attrs: Some(&["cite", "datetime"]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<del>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/del>
pub(super) static DEL_SPEC: ElementSpec = ElementSpec {
	tag: "del",
	required_attrs: &[],
	allowed_attrs: Some(&["cite", "datetime"]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<ruby>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/ruby>
pub(super) static RUBY_SPEC: ElementSpec = ElementSpec {
	tag: "ruby",
	required_attrs: &[],
	allowed_attrs: Some(&[]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<rt>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/rt>
pub(super) static RT_SPEC: ElementSpec = ElementSpec {
	tag: "rt",
	required_attrs: &[],
	allowed_attrs: Some(&[]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<rp>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/rp>
pub(super) static RP_SPEC: ElementSpec = ElementSpec {
	tag: "rp",
	required_attrs: &[],
	allowed_attrs: Some(&[]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<bdi>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/bdi>
pub(super) static BDI_SPEC: ElementSpec = ElementSpec {
	tag: "bdi",
	required_attrs: &[],
	allowed_attrs: Some(&[]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<bdo>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/bdo>
pub(super) static BDO_SPEC: ElementSpec = ElementSpec {
	tag: "bdo",
	required_attrs: &[],
	allowed_attrs: Some(&[]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<address>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/address>
pub(super) static ADDRESS_SPEC: ElementSpec = ElementSpec {
	tag: "address",
	required_attrs: &[],
	allowed_attrs: Some(&[]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<figure>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/figure>
pub(super) static FIGURE_SPEC: ElementSpec = ElementSpec {
	tag: "figure",
	required_attrs: &[],
	allowed_attrs: Some(&[]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<figcaption>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/figcaption>
pub(super) static FIGCAPTION_SPEC: ElementSpec = ElementSpec {
	tag: "figcaption",
	required_attrs: &[],
	allowed_attrs: Some(&[]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<details>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/details>
pub(super) static DETAILS_SPEC: ElementSpec = ElementSpec {
	tag: "details",
	required_attrs: &[],
	allowed_attrs: Some(&["open"]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<summary>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/summary>
pub(super) static SUMMARY_SPEC: ElementSpec = ElementSpec {
	tag: "summary",
	required_attrs: &[],
	allowed_attrs: Some(&[]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<dialog>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/dialog>
pub(super) static DIALOG_SPEC: ElementSpec = ElementSpec {
	tag: "dialog",
	required_attrs: &[],
	allowed_attrs: Some(&["open"]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<template>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/template>
pub(super) static TEMPLATE_SPEC: ElementSpec = ElementSpec {
	tag: "template",
	required_attrs: &[],
	allowed_attrs: Some(&[]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Specification for `<slot>` element.
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/slot>
pub(super) static SLOT_SPEC: ElementSpec = ElementSpec {
	tag: "slot",
	required_attrs: &[],
	allowed_attrs: Some(&["name"]),
	is_void: false,
	is_interactive: false,
	content_model: None,
};

/// Returns the element specification for a given tag name.
///
/// # Arguments
///
/// * `tag` - The HTML tag name
///
/// # Returns
///
/// An optional reference to the element specification. Returns `None` if the
/// element is not in the high-priority list.
pub(crate) fn get_element_spec(tag: &str) -> Option<&'static ElementSpec> {
	match tag {
		// High priority elements
		"img" => Some(&IMG_SPEC),
		"a" => Some(&A_SPEC),
		"button" => Some(&BUTTON_SPEC),
		"input" => Some(&INPUT_SPEC),
		"form" => Some(&FORM_SPEC),
		"label" => Some(&LABEL_SPEC),
		"select" => Some(&SELECT_SPEC),
		"textarea" => Some(&TEXTAREA_SPEC),
		"option" => Some(&OPTION_SPEC),
		// Medium priority elements
		"div" => Some(&DIV_SPEC),
		"span" => Some(&SPAN_SPEC),
		"p" => Some(&P_SPEC),
		"h1" => Some(&H1_SPEC),
		"h2" => Some(&H2_SPEC),
		"h3" => Some(&H3_SPEC),
		"h4" => Some(&H4_SPEC),
		"h5" => Some(&H5_SPEC),
		"h6" => Some(&H6_SPEC),
		"header" => Some(&HEADER_SPEC),
		"footer" => Some(&FOOTER_SPEC),
		"main" => Some(&MAIN_SPEC),
		"nav" => Some(&NAV_SPEC),
		"section" => Some(&SECTION_SPEC),
		"article" => Some(&ARTICLE_SPEC),
		// Text-level elements
		"em" => Some(&EM_SPEC),
		"strong" => Some(&STRONG_SPEC),
		"small" => Some(&SMALL_SPEC),
		"code" => Some(&CODE_SPEC),
		"kbd" => Some(&KBD_SPEC),
		"samp" => Some(&SAMP_SPEC),
		"var" => Some(&VAR_SPEC),
		"i" => Some(&I_SPEC),
		"b" => Some(&B_SPEC),
		"u" => Some(&U_SPEC),
		"mark" => Some(&MARK_SPEC),
		"s" => Some(&S_SPEC),
		"sub" => Some(&SUB_SPEC),
		"sup" => Some(&SUP_SPEC),
		"br" => Some(&BR_SPEC),
		"wbr" => Some(&WBR_SPEC),
		// List elements
		"ul" => Some(&UL_SPEC),
		"ol" => Some(&OL_SPEC),
		"li" => Some(&LI_SPEC),
		"dl" => Some(&DL_SPEC),
		"dt" => Some(&DT_SPEC),
		"dd" => Some(&DD_SPEC),
		// Table elements
		"table" => Some(&TABLE_SPEC),
		"thead" => Some(&THEAD_SPEC),
		"tbody" => Some(&TBODY_SPEC),
		"tfoot" => Some(&TFOOT_SPEC),
		"tr" => Some(&TR_SPEC),
		"th" => Some(&TH_SPEC),
		"td" => Some(&TD_SPEC),
		"caption" => Some(&CAPTION_SPEC),
		"colgroup" => Some(&COLGROUP_SPEC),
		"col" => Some(&COL_SPEC),
		// Embedded content elements
		"iframe" => Some(&IFRAME_SPEC),
		"video" => Some(&VIDEO_SPEC),
		"audio" => Some(&AUDIO_SPEC),
		"source" => Some(&SOURCE_SPEC),
		"track" => Some(&TRACK_SPEC),
		"canvas" => Some(&CANVAS_SPEC),
		"script" => Some(&SCRIPT_SPEC),
		"noscript" => Some(&NOSCRIPT_SPEC),
		"embed" => Some(&EMBED_SPEC),
		"object" => Some(&OBJECT_SPEC),
		"param" => Some(&PARAM_SPEC),
		"picture" => Some(&PICTURE_SPEC),
		// Other important elements
		"hr" => Some(&HR_SPEC),
		"pre" => Some(&PRE_SPEC),
		"blockquote" => Some(&BLOCKQUOTE_SPEC),
		"q" => Some(&Q_SPEC),
		"cite" => Some(&CITE_SPEC),
		"abbr" => Some(&ABBR_SPEC),
		"time" => Some(&TIME_SPEC),
		"data" => Some(&DATA_SPEC),
		"dfn" => Some(&DFN_SPEC),
		"ins" => Some(&INS_SPEC),
		"del" => Some(&DEL_SPEC),
		"ruby" => Some(&RUBY_SPEC),
		"rt" => Some(&RT_SPEC),
		"rp" => Some(&RP_SPEC),
		"bdi" => Some(&BDI_SPEC),
		"bdo" => Some(&BDO_SPEC),
		"address" => Some(&ADDRESS_SPEC),
		"figure" => Some(&FIGURE_SPEC),
		"figcaption" => Some(&FIGCAPTION_SPEC),
		"details" => Some(&DETAILS_SPEC),
		"summary" => Some(&SUMMARY_SPEC),
		"dialog" => Some(&DIALOG_SPEC),
		"template" => Some(&TEMPLATE_SPEC),
		"slot" => Some(&SLOT_SPEC),
		// SVG elements
		"svg" => Some(&SVG_SPEC),
		"path" => Some(&PATH_SPEC),
		"circle" => Some(&CIRCLE_SPEC),
		"rect" => Some(&RECT_SPEC),
		"line" => Some(&LINE_SPEC),
		"polyline" => Some(&POLYLINE_SPEC),
		"polygon" => Some(&POLYGON_SPEC),
		"ellipse" => Some(&ELLIPSE_SPEC),
		"g" => Some(&G_SPEC),
		"defs" => Some(&DEFS_SPEC),
		"use" => Some(&USE_SPEC),
		"symbol" => Some(&SYMBOL_SPEC),
		"text" => Some(&TEXT_SVG_SPEC),
		"tspan" => Some(&TSPAN_SPEC),
		"clipPath" => Some(&CLIPPATH_SPEC),
		"mask" => Some(&MASK_SPEC),
		"linearGradient" => Some(&LINEAR_GRADIENT_SPEC),
		"radialGradient" => Some(&RADIAL_GRADIENT_SPEC),
		"stop" => Some(&STOP_SPEC),
		"pattern" => Some(&PATTERN_SPEC),
		"image" => Some(&IMAGE_SVG_SPEC),
		"foreignObject" => Some(&FOREIGN_OBJECT_SPEC),
		"marker" => Some(&MARKER_SPEC),
		"desc" => Some(&DESC_SPEC),
		// Note: "title" is handled by HTML TITLE_SVG_SPEC in SVG context
		// Other elements not yet implemented
		_ => None,
	}
}

/// Validates an element against its HTML specification.
///
/// This function performs the following checks:
/// 1. Required attributes are present
/// 2. All attributes are allowed (if allowlist is defined)
/// 3. Content model constraints are satisfied
///
/// # Arguments
///
/// * `element` - The typed element to validate
///
/// # Errors
///
/// Returns a compile error if any validation rule is violated.
pub(crate) fn validate_against_spec(element: &TypedPageElement) -> Result<()> {
	let tag = element.tag.to_string();

	// Get specification for this element (if it exists)
	let Some(spec) = get_element_spec(&tag) else {
		// No spec defined for this element, skip validation
		return Ok(());
	};

	// Check required attributes
	validate_required_attributes(element, spec)?;

	// Check allowed attributes (if allowlist is defined)
	if let Some(allowed) = spec.allowed_attrs {
		validate_allowed_attributes(element, allowed)?;
	}

	// Check content model (void elements, text-only, etc.)
	validate_content_model(element, spec)?;

	Ok(())
}

/// Validates that all required attributes are present.
fn validate_required_attributes(element: &TypedPageElement, spec: &ElementSpec) -> Result<()> {
	for required_attr in spec.required_attrs {
		if !required_attr.required {
			continue;
		}

		let attr_found = element.attrs.iter().any(|attr| {
			let attr_name = attr.name.to_string();
			// Remove raw identifier prefix (r#type -> type)
			let attr_name = attr_name.strip_prefix("r#").unwrap_or(&attr_name);
			// Handle underscore-to-hyphen conversion
			let html_name = attr_name.replace('_', "-");
			html_name == required_attr.name
		});

		if !attr_found {
			return Err(syn::Error::new(
				element.span,
				format!(
					"Element <{}> requires '{}' attribute",
					spec.tag, required_attr.name
				),
			));
		}
	}

	Ok(())
}

/// Validates that all attributes are in the allowed list.
fn validate_allowed_attributes(element: &TypedPageElement, allowed: &[&str]) -> Result<()> {
	for attr in &element.attrs {
		let attr_name = attr.name.to_string();
		// Remove raw identifier prefix (r#type -> type)
		let attr_name = attr_name.strip_prefix("r#").unwrap_or(&attr_name);
		let html_name = attr_name.replace('_', "-");

		// Check if it's a global attribute
		if is_global_attribute(&html_name) {
			continue;
		}

		// Check if it's in the allowed list
		if !allowed.contains(&html_name.as_str()) {
			return Err(syn::Error::new(
				attr.span,
				format!(
					"Attribute '{}' is not allowed on <{}>",
					html_name, element.tag
				),
			));
		}
	}

	Ok(())
}

/// Validates content model constraints.
fn validate_content_model(element: &TypedPageElement, spec: &ElementSpec) -> Result<()> {
	match &spec.content_model {
		Some(ContentModel::Empty) => {
			if !element.children.is_empty() {
				return Err(syn::Error::new(
					element.span,
					format!("Element <{}> cannot have children (void element)", spec.tag),
				));
			}
		}
		Some(ContentModel::TextOnly) => {
			// Text-only elements can have text and expressions, but not other elements
			for child in &element.children {
				if matches!(child, reinhardt_manouche::core::TypedPageNode::Element(_)) {
					return Err(syn::Error::new(
						element.span,
						format!(
							"Element <{}> can only contain text, not child elements",
							spec.tag
						),
					));
				}
			}
		}
		Some(ContentModel::OnlyTags(allowed_tags)) => {
			// Check that all child elements are in the allowed list
			for child in &element.children {
				if let reinhardt_manouche::core::TypedPageNode::Element(child_elem) = child {
					let child_tag = child_elem.tag.to_string();
					if !allowed_tags.contains(&child_tag.as_str()) {
						return Err(syn::Error::new(
							child_elem.span,
							format!(
								"Element <{}> can only contain: {}",
								spec.tag,
								allowed_tags.join(", ")
							),
						));
					}
				}
			}
		}
		None => {
			// No content model constraint
		}
	}

	Ok(())
}

/// Checks if an attribute is a global HTML attribute.
fn is_global_attribute(attr: &str) -> bool {
	// Check standard global attributes
	if GLOBAL_ATTRS.contains(&attr) {
		return true;
	}

	// Check data-* attributes
	if attr.starts_with("data-") {
		return true;
	}

	// Check aria-* attributes
	if attr.starts_with("aria-") {
		return true;
	}

	false
}

// ========================================
// Enumerated Attributes Specifications
// ========================================

/// Enumerated attributes for input element.
///
/// Based on: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/input#type>
pub(super) static INPUT_ENUM_ATTRS: ElementEnumAttrs = ElementEnumAttrs {
	tag: "input",
	attrs: &[EnumAttrSpec {
		name: "type",
		valid_values: &[
			"text",
			"password",
			"email",
			"number",
			"tel",
			"url",
			"search",
			"checkbox",
			"radio",
			"submit",
			"button",
			"reset",
			"file",
			"hidden",
			"date",
			"datetime-local",
			"time",
			"week",
			"month",
			"color",
			"range",
			"image",
		],
	}],
};

/// Enumerated attributes for button element.
///
/// Based on: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/button#type>
pub(super) static BUTTON_ENUM_ATTRS: ElementEnumAttrs = ElementEnumAttrs {
	tag: "button",
	attrs: &[EnumAttrSpec {
		name: "type",
		valid_values: &["submit", "button", "reset"],
	}],
};

/// Enumerated attributes for form element.
///
/// Based on: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/form>
pub(super) static FORM_ENUM_ATTRS: ElementEnumAttrs = ElementEnumAttrs {
	tag: "form",
	attrs: &[
		EnumAttrSpec {
			name: "method",
			valid_values: &["get", "post", "dialog"],
		},
		EnumAttrSpec {
			name: "enctype",
			valid_values: &[
				"application/x-www-form-urlencoded",
				"multipart/form-data",
				"text/plain",
			],
		},
	],
};

/// Enumerated attributes for script element.
///
/// Based on: <https://developer.mozilla.org/en-US/docs/Web/HTML/Element/script#type>
pub(super) static SCRIPT_ENUM_ATTRS: ElementEnumAttrs = ElementEnumAttrs {
	tag: "script",
	attrs: &[EnumAttrSpec {
		name: "type",
		valid_values: &["module", "text/javascript", "application/javascript"],
	}],
};

/// Gets enumerated attribute specification for an element/attribute pair.
///
/// Returns `None` if the element does not have enumerated attributes or
/// the specific attribute is not enumerated.
///
/// # Parameters
///
/// * `tag` - The element tag name
/// * `attr` - The attribute name
///
/// # Returns
///
/// The enumerated attribute specification if found, otherwise `None`.
pub(crate) fn get_enum_attr_spec(tag: &str, attr: &str) -> Option<&'static EnumAttrSpec> {
	let element_attrs = match tag {
		"input" => &INPUT_ENUM_ATTRS,
		"button" => &BUTTON_ENUM_ATTRS,
		"form" => &FORM_ENUM_ATTRS,
		"script" => &SCRIPT_ENUM_ATTRS,
		_ => return None,
	};

	element_attrs.attrs.iter().find(|spec| spec.name == attr)
}

// ============================================================================
// SVG Element Specifications
// ============================================================================
//
// SVG (Scalable Vector Graphics) elements for inline vector graphics support.
// These specifications enable form! and page! macros to include SVG icons
// and graphics with compile-time validation.
//
// Reference: https://developer.mozilla.org/en-US/docs/Web/SVG/Element

/// SVG presentation attributes (allowed on most SVG elements).
///
/// Based on: <https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/Presentation>
#[allow(dead_code)] // Will be used by form! macro SVG icon validation
pub(crate) static SVG_PRESENTATION_ATTRS: &[&str] = &[
	// Fill and stroke
	"fill",
	"fill-opacity",
	"fill-rule",
	"stroke",
	"stroke-dasharray",
	"stroke-dashoffset",
	"stroke-linecap",
	"stroke-linejoin",
	"stroke-miterlimit",
	"stroke-opacity",
	"stroke-width",
	// Color and paint
	"color",
	"opacity",
	"paint-order",
	// Transform
	"transform",
	"transform-origin",
	// Text
	"font-family",
	"font-size",
	"font-style",
	"font-weight",
	"text-anchor",
	"text-decoration",
	// Visibility
	"visibility",
	"display",
	// Clipping and masking
	"clip-path",
	"clip-rule",
	"mask",
	// Filter
	"filter",
	// Markers
	"marker-start",
	"marker-mid",
	"marker-end",
];

/// Specification for `<svg>` element (SVG root container).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/svg>
pub(super) static SVG_SPEC: ElementSpec = ElementSpec {
	tag: "svg",
	required_attrs: &[],
	allowed_attrs: Some(&[
		// Sizing and viewport
		"width",
		"height",
		"viewBox",
		"preserveAspectRatio",
		// Namespace (usually implicit in HTML5)
		"xmlns",
		"xmlns:xlink",
		// Common presentation attributes (subset)
		"fill",
		"stroke",
		"stroke-width",
		"transform",
		"opacity",
		// Other
		"x",
		"y",
	]),
	is_void: false,
	is_interactive: false,
	content_model: None, // SVG content
};

/// Specification for `<path>` element (SVG path).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/path>
pub(super) static PATH_SPEC: ElementSpec = ElementSpec {
	tag: "path",
	required_attrs: &[],
	allowed_attrs: Some(&[
		// Path data (required for rendering)
		"d",
		// Fill and stroke
		"fill",
		"fill-opacity",
		"fill-rule",
		"stroke",
		"stroke-width",
		"stroke-linecap",
		"stroke-linejoin",
		"stroke-dasharray",
		"stroke-dashoffset",
		"stroke-opacity",
		// Transform
		"transform",
		// Markers
		"marker-start",
		"marker-mid",
		"marker-end",
		// Clip and mask
		"clip-path",
		"mask",
		// Other
		"opacity",
		"visibility",
		"pathLength",
	]),
	is_void: true, // path has no children
	is_interactive: false,
	content_model: Some(ContentModel::Empty),
};

/// Specification for `<circle>` element (SVG circle).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/circle>
pub(super) static CIRCLE_SPEC: ElementSpec = ElementSpec {
	tag: "circle",
	required_attrs: &[],
	allowed_attrs: Some(&[
		// Geometry
		"cx",
		"cy",
		"r",
		// Fill and stroke
		"fill",
		"fill-opacity",
		"stroke",
		"stroke-width",
		"stroke-dasharray",
		"stroke-opacity",
		// Transform
		"transform",
		// Other
		"opacity",
		"visibility",
		"pathLength",
	]),
	is_void: true,
	is_interactive: false,
	content_model: Some(ContentModel::Empty),
};

/// Specification for `<rect>` element (SVG rectangle).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/rect>
pub(super) static RECT_SPEC: ElementSpec = ElementSpec {
	tag: "rect",
	required_attrs: &[],
	allowed_attrs: Some(&[
		// Geometry
		"x",
		"y",
		"width",
		"height",
		"rx",
		"ry",
		// Fill and stroke
		"fill",
		"fill-opacity",
		"stroke",
		"stroke-width",
		"stroke-dasharray",
		"stroke-opacity",
		// Transform
		"transform",
		// Other
		"opacity",
		"visibility",
		"pathLength",
	]),
	is_void: true,
	is_interactive: false,
	content_model: Some(ContentModel::Empty),
};

/// Specification for `<line>` element (SVG line).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/line>
pub(super) static LINE_SPEC: ElementSpec = ElementSpec {
	tag: "line",
	required_attrs: &[],
	allowed_attrs: Some(&[
		// Geometry
		"x1",
		"y1",
		"x2",
		"y2",
		// Stroke (fill doesn't apply to lines)
		"stroke",
		"stroke-width",
		"stroke-linecap",
		"stroke-dasharray",
		"stroke-opacity",
		// Transform
		"transform",
		// Markers
		"marker-start",
		"marker-end",
		// Other
		"opacity",
		"visibility",
		"pathLength",
	]),
	is_void: true,
	is_interactive: false,
	content_model: Some(ContentModel::Empty),
};

/// Specification for `<polyline>` element (SVG polyline).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/polyline>
pub(super) static POLYLINE_SPEC: ElementSpec = ElementSpec {
	tag: "polyline",
	required_attrs: &[],
	allowed_attrs: Some(&[
		// Points (required for rendering)
		"points",
		// Fill and stroke
		"fill",
		"fill-opacity",
		"fill-rule",
		"stroke",
		"stroke-width",
		"stroke-linecap",
		"stroke-linejoin",
		"stroke-dasharray",
		"stroke-opacity",
		// Transform
		"transform",
		// Markers
		"marker-start",
		"marker-mid",
		"marker-end",
		// Other
		"opacity",
		"visibility",
		"pathLength",
	]),
	is_void: true,
	is_interactive: false,
	content_model: Some(ContentModel::Empty),
};

/// Specification for `<polygon>` element (SVG polygon).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/polygon>
pub(super) static POLYGON_SPEC: ElementSpec = ElementSpec {
	tag: "polygon",
	required_attrs: &[],
	allowed_attrs: Some(&[
		// Points (required for rendering)
		"points",
		// Fill and stroke
		"fill",
		"fill-opacity",
		"fill-rule",
		"stroke",
		"stroke-width",
		"stroke-linecap",
		"stroke-linejoin",
		"stroke-dasharray",
		"stroke-opacity",
		// Transform
		"transform",
		// Markers
		"marker-start",
		"marker-mid",
		"marker-end",
		// Other
		"opacity",
		"visibility",
		"pathLength",
	]),
	is_void: true,
	is_interactive: false,
	content_model: Some(ContentModel::Empty),
};

/// Specification for `<ellipse>` element (SVG ellipse).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/ellipse>
pub(super) static ELLIPSE_SPEC: ElementSpec = ElementSpec {
	tag: "ellipse",
	required_attrs: &[],
	allowed_attrs: Some(&[
		// Geometry
		"cx",
		"cy",
		"rx",
		"ry",
		// Fill and stroke
		"fill",
		"fill-opacity",
		"stroke",
		"stroke-width",
		"stroke-dasharray",
		"stroke-opacity",
		// Transform
		"transform",
		// Other
		"opacity",
		"visibility",
		"pathLength",
	]),
	is_void: true,
	is_interactive: false,
	content_model: Some(ContentModel::Empty),
};

/// Specification for `<g>` element (SVG group container).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/g>
pub(super) static G_SPEC: ElementSpec = ElementSpec {
	tag: "g",
	required_attrs: &[],
	allowed_attrs: Some(&[
		// Fill and stroke (inherited by children)
		"fill",
		"fill-opacity",
		"fill-rule",
		"stroke",
		"stroke-width",
		"stroke-linecap",
		"stroke-linejoin",
		"stroke-dasharray",
		"stroke-opacity",
		// Transform
		"transform",
		// Clip and mask
		"clip-path",
		"mask",
		// Filter
		"filter",
		// Other
		"opacity",
		"visibility",
	]),
	is_void: false,
	is_interactive: false,
	content_model: None, // SVG content
};

/// Specification for `<defs>` element (SVG definitions container).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/defs>
pub(super) static DEFS_SPEC: ElementSpec = ElementSpec {
	tag: "defs",
	required_attrs: &[],
	allowed_attrs: None, // All attributes allowed (it's a container)
	is_void: false,
	is_interactive: false,
	content_model: None, // SVG content (gradients, patterns, symbols, etc.)
};

/// Specification for `<use>` element (SVG use/reference).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/use>
pub(super) static USE_SPEC: ElementSpec = ElementSpec {
	tag: "use",
	required_attrs: &[],
	allowed_attrs: Some(&[
		// Reference
		"href",
		"xlink:href", // Deprecated but still used
		// Position and size
		"x",
		"y",
		"width",
		"height",
		// Fill and stroke
		"fill",
		"stroke",
		"stroke-width",
		// Transform
		"transform",
		// Other
		"opacity",
		"visibility",
	]),
	is_void: true,
	is_interactive: false,
	content_model: Some(ContentModel::Empty),
};

/// Specification for `<symbol>` element (SVG symbol definition).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/symbol>
pub(super) static SYMBOL_SPEC: ElementSpec = ElementSpec {
	tag: "symbol",
	required_attrs: &[],
	allowed_attrs: Some(&[
		// Viewport
		"viewBox",
		"preserveAspectRatio",
		// Sizing
		"x",
		"y",
		"width",
		"height",
		// Reference (for identification)
		"refX",
		"refY",
	]),
	is_void: false,
	is_interactive: false,
	content_model: None, // SVG content
};

/// Specification for `<text>` element (SVG text).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/text>
pub(super) static TEXT_SVG_SPEC: ElementSpec = ElementSpec {
	tag: "text",
	required_attrs: &[],
	allowed_attrs: Some(&[
		// Position
		"x",
		"y",
		"dx",
		"dy",
		// Text layout
		"textLength",
		"lengthAdjust",
		"rotate",
		// Font
		"font-family",
		"font-size",
		"font-style",
		"font-weight",
		"text-anchor",
		"text-decoration",
		"dominant-baseline",
		// Fill and stroke
		"fill",
		"fill-opacity",
		"stroke",
		"stroke-width",
		// Transform
		"transform",
		// Other
		"opacity",
		"visibility",
	]),
	is_void: false,
	is_interactive: false,
	content_model: None, // Text content and tspan elements
};

/// Specification for `<tspan>` element (SVG text span).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/tspan>
pub(super) static TSPAN_SPEC: ElementSpec = ElementSpec {
	tag: "tspan",
	required_attrs: &[],
	allowed_attrs: Some(&[
		// Position
		"x",
		"y",
		"dx",
		"dy",
		// Text layout
		"textLength",
		"lengthAdjust",
		"rotate",
		// Font
		"font-family",
		"font-size",
		"font-style",
		"font-weight",
		// Fill and stroke
		"fill",
		"stroke",
		// Other
		"baseline-shift",
	]),
	is_void: false,
	is_interactive: false,
	content_model: None, // Text content
};

/// Specification for `<clipPath>` element (SVG clip path).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/clipPath>
pub(super) static CLIPPATH_SPEC: ElementSpec = ElementSpec {
	tag: "clipPath",
	required_attrs: &[],
	allowed_attrs: Some(&["clipPathUnits", "transform"]),
	is_void: false,
	is_interactive: false,
	content_model: None, // Shape elements
};

/// Specification for `<mask>` element (SVG mask).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/mask>
pub(super) static MASK_SPEC: ElementSpec = ElementSpec {
	tag: "mask",
	required_attrs: &[],
	allowed_attrs: Some(&["x", "y", "width", "height", "maskUnits", "maskContentUnits"]),
	is_void: false,
	is_interactive: false,
	content_model: None, // Any SVG content
};

/// Specification for `<linearGradient>` element (SVG linear gradient).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/linearGradient>
pub(super) static LINEAR_GRADIENT_SPEC: ElementSpec = ElementSpec {
	tag: "linearGradient",
	required_attrs: &[],
	allowed_attrs: Some(&[
		"x1",
		"y1",
		"x2",
		"y2",
		"gradientUnits",
		"gradientTransform",
		"spreadMethod",
		"href",
		"xlink:href",
	]),
	is_void: false,
	is_interactive: false,
	content_model: Some(ContentModel::OnlyTags(&["stop"])),
};

/// Specification for `<radialGradient>` element (SVG radial gradient).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/radialGradient>
pub(super) static RADIAL_GRADIENT_SPEC: ElementSpec = ElementSpec {
	tag: "radialGradient",
	required_attrs: &[],
	allowed_attrs: Some(&[
		"cx",
		"cy",
		"r",
		"fx",
		"fy",
		"fr",
		"gradientUnits",
		"gradientTransform",
		"spreadMethod",
		"href",
		"xlink:href",
	]),
	is_void: false,
	is_interactive: false,
	content_model: Some(ContentModel::OnlyTags(&["stop"])),
};

/// Specification for `<stop>` element (SVG gradient stop).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/stop>
pub(super) static STOP_SPEC: ElementSpec = ElementSpec {
	tag: "stop",
	required_attrs: &[],
	allowed_attrs: Some(&["offset", "stop-color", "stop-opacity"]),
	is_void: true,
	is_interactive: false,
	content_model: Some(ContentModel::Empty),
};

/// Specification for `<pattern>` element (SVG pattern).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/pattern>
pub(super) static PATTERN_SPEC: ElementSpec = ElementSpec {
	tag: "pattern",
	required_attrs: &[],
	allowed_attrs: Some(&[
		"x",
		"y",
		"width",
		"height",
		"patternUnits",
		"patternContentUnits",
		"patternTransform",
		"viewBox",
		"preserveAspectRatio",
		"href",
		"xlink:href",
	]),
	is_void: false,
	is_interactive: false,
	content_model: None, // Any SVG content
};

/// Specification for `<image>` element (SVG image).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/image>
pub(super) static IMAGE_SVG_SPEC: ElementSpec = ElementSpec {
	tag: "image",
	required_attrs: &[],
	allowed_attrs: Some(&[
		"href",
		"xlink:href",
		"x",
		"y",
		"width",
		"height",
		"preserveAspectRatio",
		"crossorigin",
		"decoding",
		// Transform
		"transform",
		// Clip
		"clip-path",
		// Other
		"opacity",
		"visibility",
	]),
	is_void: true,
	is_interactive: false,
	content_model: Some(ContentModel::Empty),
};

/// Specification for `<foreignObject>` element (SVG foreign object for embedding HTML).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/foreignObject>
pub(super) static FOREIGN_OBJECT_SPEC: ElementSpec = ElementSpec {
	tag: "foreignObject",
	required_attrs: &[],
	allowed_attrs: Some(&[
		"x",
		"y",
		"width",
		"height",
		// Transform
		"transform",
		// Other
		"opacity",
		"visibility",
	]),
	is_void: false,
	is_interactive: false,
	content_model: None, // HTML content
};

/// Specification for `<marker>` element (SVG marker).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/marker>
pub(super) static MARKER_SPEC: ElementSpec = ElementSpec {
	tag: "marker",
	required_attrs: &[],
	allowed_attrs: Some(&[
		"markerWidth",
		"markerHeight",
		"refX",
		"refY",
		"orient",
		"markerUnits",
		"viewBox",
		"preserveAspectRatio",
		// Fill and stroke (for marker contents)
		"fill",
		"stroke",
		"stroke-width",
	]),
	is_void: false,
	is_interactive: false,
	content_model: None, // SVG content
};

/// Specification for `<title>` element in SVG context (accessible title).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/title>
#[allow(dead_code)] // Will be used by form! macro SVG icon validation
pub(super) static TITLE_SVG_SPEC: ElementSpec = ElementSpec {
	tag: "title",
	required_attrs: &[],
	allowed_attrs: None,
	is_void: false,
	is_interactive: false,
	content_model: Some(ContentModel::TextOnly),
};

/// Specification for `<desc>` element (SVG description for accessibility).
///
/// Reference: <https://developer.mozilla.org/en-US/docs/Web/SVG/Element/desc>
pub(super) static DESC_SPEC: ElementSpec = ElementSpec {
	tag: "desc",
	required_attrs: &[],
	allowed_attrs: None,
	is_void: false,
	is_interactive: false,
	content_model: Some(ContentModel::TextOnly),
};

/// Checks if a tag is an SVG element.
///
/// This is used to determine if SVG-specific validation rules should apply.
#[allow(dead_code)] // Will be used by form! macro SVG icon validation
pub(crate) fn is_svg_element(tag: &str) -> bool {
	matches!(
		tag,
		"svg"
			| "path" | "circle"
			| "rect" | "line"
			| "polyline"
			| "polygon"
			| "ellipse"
			| "g" | "defs"
			| "use" | "symbol"
			| "text" | "tspan"
			| "clipPath"
			| "mask" | "linearGradient"
			| "radialGradient"
			| "stop" | "pattern"
			| "image" | "foreignObject"
			| "marker"
			| "title" | "desc"
	)
}

/// Checks if an attribute is a valid SVG presentation attribute.
///
/// Presentation attributes can be used on most SVG elements.
#[allow(dead_code)] // Will be used by form! macro SVG icon validation
pub(crate) fn is_svg_presentation_attr(attr: &str) -> bool {
	SVG_PRESENTATION_ATTRS.contains(&attr)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_get_element_spec() {
		assert!(get_element_spec("img").is_some());
		assert!(get_element_spec("a").is_some());
		assert!(get_element_spec("button").is_some());
		assert!(get_element_spec("div").is_some());
	}

	#[test]
	fn test_is_global_attribute() {
		assert!(is_global_attribute("id"));
		assert!(is_global_attribute("class"));
		assert!(is_global_attribute("data-testid"));
		assert!(is_global_attribute("aria-label"));
		assert!(!is_global_attribute("href"));
	}

	#[test]
	fn test_img_spec() {
		assert_eq!(IMG_SPEC.tag, "img");
		assert_eq!(IMG_SPEC.required_attrs.len(), 2);
		assert!(IMG_SPEC.is_void);
		assert!(!IMG_SPEC.is_interactive);
	}

	#[test]
	fn test_select_content_model() {
		let spec = get_element_spec("select").unwrap();
		match &spec.content_model {
			Some(ContentModel::OnlyTags(tags)) => {
				assert!(tags.contains(&"option"));
				assert!(tags.contains(&"optgroup"));
			}
			_ => panic!("Expected OnlyTags content model"),
		}
	}

	// SVG Element Tests

	#[test]
	fn test_get_svg_element_spec() {
		// Basic SVG elements
		assert!(get_element_spec("svg").is_some());
		assert!(get_element_spec("path").is_some());
		assert!(get_element_spec("circle").is_some());
		assert!(get_element_spec("rect").is_some());
		assert!(get_element_spec("line").is_some());
		assert!(get_element_spec("polyline").is_some());
		assert!(get_element_spec("polygon").is_some());
		assert!(get_element_spec("ellipse").is_some());

		// Container elements
		assert!(get_element_spec("g").is_some());
		assert!(get_element_spec("defs").is_some());
		assert!(get_element_spec("symbol").is_some());
		assert!(get_element_spec("use").is_some());

		// Text elements
		assert!(get_element_spec("text").is_some());
		assert!(get_element_spec("tspan").is_some());

		// Gradient elements
		assert!(get_element_spec("linearGradient").is_some());
		assert!(get_element_spec("radialGradient").is_some());
		assert!(get_element_spec("stop").is_some());

		// Other SVG elements
		assert!(get_element_spec("clipPath").is_some());
		assert!(get_element_spec("mask").is_some());
		assert!(get_element_spec("pattern").is_some());
		assert!(get_element_spec("marker").is_some());
		assert!(get_element_spec("foreignObject").is_some());
		assert!(get_element_spec("desc").is_some());
	}

	#[test]
	fn test_is_svg_element() {
		// Should return true for SVG elements
		assert!(is_svg_element("svg"));
		assert!(is_svg_element("path"));
		assert!(is_svg_element("circle"));
		assert!(is_svg_element("g"));
		assert!(is_svg_element("linearGradient"));

		// Should return false for HTML elements
		assert!(!is_svg_element("div"));
		assert!(!is_svg_element("span"));
		assert!(!is_svg_element("img"));
	}

	#[test]
	fn test_svg_spec() {
		let spec = get_element_spec("svg").unwrap();
		assert_eq!(spec.tag, "svg");
		assert!(!spec.is_void);
		assert!(!spec.is_interactive);

		// Check allowed attributes
		if let Some(allowed) = spec.allowed_attrs {
			assert!(allowed.contains(&"viewBox"));
			assert!(allowed.contains(&"width"));
			assert!(allowed.contains(&"height"));
			assert!(allowed.contains(&"fill"));
		}
	}

	#[test]
	fn test_path_spec_void() {
		let spec = get_element_spec("path").unwrap();
		assert_eq!(spec.tag, "path");
		assert!(spec.is_void);
		assert!(matches!(spec.content_model, Some(ContentModel::Empty)));

		// Check d attribute is allowed
		if let Some(allowed) = spec.allowed_attrs {
			assert!(allowed.contains(&"d"));
			assert!(allowed.contains(&"fill"));
			assert!(allowed.contains(&"stroke"));
		}
	}

	#[test]
	fn test_gradient_content_model() {
		let linear_spec = get_element_spec("linearGradient").unwrap();
		match &linear_spec.content_model {
			Some(ContentModel::OnlyTags(tags)) => {
				assert!(tags.contains(&"stop"));
				assert_eq!(tags.len(), 1);
			}
			_ => panic!("Expected OnlyTags content model for linearGradient"),
		}

		let radial_spec = get_element_spec("radialGradient").unwrap();
		match &radial_spec.content_model {
			Some(ContentModel::OnlyTags(tags)) => {
				assert!(tags.contains(&"stop"));
				assert_eq!(tags.len(), 1);
			}
			_ => panic!("Expected OnlyTags content model for radialGradient"),
		}
	}

	#[test]
	fn test_svg_presentation_attrs() {
		// Check common presentation attributes
		assert!(is_svg_presentation_attr("fill"));
		assert!(is_svg_presentation_attr("stroke"));
		assert!(is_svg_presentation_attr("stroke-width"));
		assert!(is_svg_presentation_attr("transform"));
		assert!(is_svg_presentation_attr("opacity"));

		// Should return false for non-presentation attributes
		assert!(!is_svg_presentation_attr("viewBox"));
		assert!(!is_svg_presentation_attr("d"));
		assert!(!is_svg_presentation_attr("cx"));
	}

	#[test]
	fn test_desc_text_only() {
		let spec = get_element_spec("desc").unwrap();
		assert!(matches!(spec.content_model, Some(ContentModel::TextOnly)));
	}
}
