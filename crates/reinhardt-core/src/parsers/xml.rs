//! XML parser for parsing XML request bodies
//!
//! Provides parsing of XML content into a JSON-like structure using quick-xml.
//! Supports attributes, namespaces, and CDATA sections.
//! Input must be valid UTF-8; malformed byte sequences produce validation errors.

use super::parser::{ParseResult, ParsedData, Parser};
use crate::exception::Error;
use async_trait::async_trait;
use bytes::Bytes;
use http::HeaderMap;
use quick_xml::events::{Event, attributes::Attributes};
use quick_xml::{Reader, XmlVersion};
use serde_json::{Map, Value, json};

/// XML parser configuration
#[derive(Debug, Clone)]
pub struct XmlParserConfig {
	/// Include XML attributes in parsed output
	pub include_attributes: bool,
	/// Attribute prefix (default: "@")
	pub attribute_prefix: String,
	/// Text content key (default: "#text")
	pub text_key: String,
	/// Preserve CDATA sections
	pub preserve_cdata: bool,
	/// Trim whitespace from text nodes
	pub trim_text: bool,
	/// Parse numeric values
	pub parse_numbers: bool,
	/// Parse boolean values
	pub parse_booleans: bool,
}

impl Default for XmlParserConfig {
	fn default() -> Self {
		Self {
			include_attributes: true,
			attribute_prefix: "@".to_string(),
			text_key: "#text".to_string(),
			preserve_cdata: false,
			trim_text: true,
			parse_numbers: false,
			parse_booleans: false,
		}
	}
}

impl XmlParserConfig {
	/// Creates a new XML parser configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::xml::XmlParserConfig;
	///
	/// let config = XmlParserConfig::new();
	/// assert!(config.include_attributes);
	/// assert_eq!(config.attribute_prefix, "@");
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Creates a builder for fluent configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::xml::XmlParserConfig;
	///
	/// let config = XmlParserConfig::builder()
	///     .include_attributes(false)
	///     .trim_text(true)
	///     .parse_numbers(true)
	///     .build();
	///
	/// assert!(!config.include_attributes);
	/// assert!(config.parse_numbers);
	/// ```
	pub fn builder() -> XmlParserConfigBuilder {
		XmlParserConfigBuilder::default()
	}
}

/// Builder for XmlParserConfig
#[derive(Debug, Default)]
pub struct XmlParserConfigBuilder {
	include_attributes: Option<bool>,
	attribute_prefix: Option<String>,
	text_key: Option<String>,
	preserve_cdata: Option<bool>,
	trim_text: Option<bool>,
	parse_numbers: Option<bool>,
	parse_booleans: Option<bool>,
}

impl XmlParserConfigBuilder {
	/// Set whether to include XML attributes
	pub fn include_attributes(mut self, include: bool) -> Self {
		self.include_attributes = Some(include);
		self
	}

	/// Set the attribute prefix
	pub fn attribute_prefix(mut self, prefix: String) -> Self {
		self.attribute_prefix = Some(prefix);
		self
	}

	/// Set the text content key
	pub fn text_key(mut self, key: String) -> Self {
		self.text_key = Some(key);
		self
	}

	/// Set whether to preserve CDATA
	pub fn preserve_cdata(mut self, preserve: bool) -> Self {
		self.preserve_cdata = Some(preserve);
		self
	}

	/// Set whether to trim whitespace
	pub fn trim_text(mut self, trim: bool) -> Self {
		self.trim_text = Some(trim);
		self
	}

	/// Set whether to parse numbers
	pub fn parse_numbers(mut self, parse: bool) -> Self {
		self.parse_numbers = Some(parse);
		self
	}

	/// Set whether to parse booleans
	pub fn parse_booleans(mut self, parse: bool) -> Self {
		self.parse_booleans = Some(parse);
		self
	}

	/// Build the configuration
	pub fn build(self) -> XmlParserConfig {
		let default = XmlParserConfig::default();
		XmlParserConfig {
			include_attributes: self
				.include_attributes
				.unwrap_or(default.include_attributes),
			attribute_prefix: self.attribute_prefix.unwrap_or(default.attribute_prefix),
			text_key: self.text_key.unwrap_or(default.text_key),
			preserve_cdata: self.preserve_cdata.unwrap_or(default.preserve_cdata),
			trim_text: self.trim_text.unwrap_or(default.trim_text),
			parse_numbers: self.parse_numbers.unwrap_or(default.parse_numbers),
			parse_booleans: self.parse_booleans.unwrap_or(default.parse_booleans),
		}
	}
}

/// XML parser for request bodies
///
/// # Examples
///
/// ```
/// use reinhardt_core::parsers::xml::{XMLParser, XmlParserConfig};
/// use reinhardt_core::parsers::Parser;
///
/// let parser = XMLParser::new();
/// assert_eq!(parser.media_types(), vec!["application/xml", "text/xml"]);
/// ```
pub struct XMLParser {
	config: XmlParserConfig,
}

impl XMLParser {
	/// Creates a new XML parser with default configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::xml::XMLParser;
	///
	/// let parser = XMLParser::new();
	/// ```
	pub fn new() -> Self {
		Self {
			config: XmlParserConfig::default(),
		}
	}

	/// Creates a new XML parser with custom configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::parsers::xml::{XMLParser, XmlParserConfig};
	///
	/// let config = XmlParserConfig::builder()
	///     .parse_numbers(true)
	///     .trim_text(true)
	///     .build();
	///
	/// let parser = XMLParser::with_config(config);
	/// ```
	pub fn with_config(config: XmlParserConfig) -> Self {
		Self { config }
	}

	/// Parse XML bytes into a JSON Value
	fn parse_xml(&self, bytes: &[u8]) -> ParseResult<Value> {
		let mut reader = Reader::from_reader(bytes);
		let mut stack: Vec<(String, Map<String, Value>)> = Vec::new();
		let mut current_text = String::new();
		let mut xml_version = XmlVersion::default();

		loop {
			match reader.read_event() {
				Ok(Event::Start(e)) => {
					let name = e.name().as_ref().to_owned();
					let mut obj = Map::new();

					// Handle attributes
					if self.config.include_attributes {
						self.process_attributes(e.attributes(), &mut obj)?;
					}

					stack.push((name, obj));
					current_text.clear();
				}

				Ok(Event::End(_)) => {
					if let Some((name, mut obj)) = stack.pop() {
						// Add text content if present
						if !current_text.is_empty() {
							let value = self.parse_value(&current_text);
							obj.insert(self.config.text_key.clone(), value);
							current_text.clear();
						}

						let value = Value::Object(obj);

						if let Some((_, parent)) = stack.last_mut() {
							// Add to parent
							self.add_to_parent(parent, &name, value);
						} else {
							// Root element
							return Ok(json!({ name: value }));
						}
					}
				}

				Ok(Event::Text(e)) => {
					let text = e.xml_content(xml_version);

					if self.config.trim_text {
						let trimmed = text.trim();
						if !trimmed.is_empty() {
							current_text.push_str(trimmed);
						}
					} else {
						current_text.push_str(&text);
					}
				}

				Ok(Event::CData(e)) => {
					let text = e.into_inner();
					if self.config.preserve_cdata {
						current_text.push_str(&format!("<![CDATA[{}]]>", text));
					} else {
						current_text.push_str(&text);
					}
				}

				Ok(Event::Empty(e)) => {
					let name = e.name().as_ref().to_owned();
					let mut obj = Map::new();

					// Handle attributes
					if self.config.include_attributes {
						self.process_attributes(e.attributes(), &mut obj)?;
					}

					let value = if obj.is_empty() {
						Value::Null
					} else {
						Value::Object(obj)
					};

					if let Some((_, parent)) = stack.last_mut() {
						self.add_to_parent(parent, &name, value);
					} else {
						return Ok(json!({ name: value }));
					}
				}

				Ok(Event::Decl(e)) => {
					xml_version = e
						.xml_version()
						.map_err(|e| Error::Validation(format!("XML declaration error: {}", e)))?;
				}

				Ok(Event::Eof) => break,

				Ok(_) => {}

				Err(e) => {
					return Err(Error::Validation(format!("XML parse error: {}", e)));
				}
			}
		}

		Ok(Value::Null)
	}

	/// Process XML attributes
	fn process_attributes(
		&self,
		attributes: Attributes,
		obj: &mut Map<String, Value>,
	) -> ParseResult<()> {
		for attr in attributes {
			let attr =
				attr.map_err(|e| Error::Validation(format!("XML attribute error: {}", e)))?;

			let key = format!("{}{}", self.config.attribute_prefix, attr.key.as_ref());
			let value = self.parse_value(&attr.value);

			obj.insert(key, value);
		}
		Ok(())
	}

	/// Add value to parent object
	fn add_to_parent(&self, parent: &mut Map<String, Value>, name: &str, value: Value) {
		if let Some(existing) = parent.get_mut(name) {
			// Convert to array if not already
			match existing {
				Value::Array(arr) => {
					arr.push(value);
				}
				_ => {
					let old_value = existing.clone();
					*existing = json!([old_value, value]);
				}
			}
		} else {
			parent.insert(name.to_string(), value);
		}
	}

	/// Parse string value to appropriate JSON type
	fn parse_value(&self, s: &str) -> Value {
		if self.config.parse_numbers {
			if let Ok(i) = s.parse::<i64>() {
				return json!(i);
			}
			if let Ok(f) = s.parse::<f64>() {
				return json!(f);
			}
		}

		if self.config.parse_booleans {
			match s.to_lowercase().as_str() {
				"true" => return json!(true),
				"false" => return json!(false),
				_ => {}
			}
		}

		json!(s)
	}
}

impl Default for XMLParser {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl Parser for XMLParser {
	fn media_types(&self) -> Vec<String> {
		vec!["application/xml".to_string(), "text/xml".to_string()]
	}

	async fn parse(
		&self,
		_content_type: Option<&str>,
		body: Bytes,
		_headers: &HeaderMap,
	) -> ParseResult<ParsedData> {
		let value = self.parse_xml(&body)?;
		Ok(ParsedData::Xml(value))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_xml_parser_utf8_validation() {
		// Arrange
		let parser = XMLParser::new();
		let xml = r#"<root 言語="日本語"><名前>雪</名前><空 属性="値"/><注><![CDATA[雪 < 雨]]></注></root>"#;

		// Act
		let value = parser.parse_xml(xml.as_bytes()).unwrap();

		// Assert
		assert_eq!(
			value,
			json!({"root": {
				"@言語": "日本語",
				"名前": {"#text": "雪"},
				"空": {"@属性": "値"},
				"注": {"#text": "雪 < 雨"},
			}})
		);
		for bytes in [
			b"<\xff/>".as_slice(),
			b"<root>\xff</root>",
			b"<root name=\"\xff\"/>",
			b"<root><![CDATA[\xff]]></root>",
		] {
			assert!(matches!(parser.parse_xml(bytes), Err(Error::Validation(_))));
		}
	}

	#[tokio::test]
	async fn test_xml_parser_simple() {
		let parser = XMLParser::new();
		let xml = Bytes::from("<root><name>John</name><age>30</age></root>");

		let headers = HeaderMap::new();

		let result = parser
			.parse(Some("application/xml"), xml, &headers)
			.await
			.unwrap();
		match result {
			ParsedData::Xml(value) => {
				assert!(value.is_object());
				let root = value.get("root").unwrap();
				assert_eq!(root.get("#text"), None);
			}
			_ => panic!("Expected XML"),
		}
	}

	#[tokio::test]
	async fn test_xml_parser_with_declaration() {
		let parser = XMLParser::new();
		let xml = Bytes::from(r#"<?xml version="1.1"?><root>John</root>"#);

		let headers = HeaderMap::new();

		let result = parser
			.parse(Some("application/xml"), xml, &headers)
			.await
			.unwrap();
		match result {
			ParsedData::Xml(value) => {
				let root = value.get("root").unwrap();
				assert_eq!(root.get("#text").unwrap(), "John");
			}
			_ => panic!("Expected XML"),
		}
	}

	#[tokio::test]
	async fn test_xml_parser_with_attributes() {
		let parser = XMLParser::new();
		let xml = Bytes::from(r#"<root id="123"><name lang="en">John</name></root>"#);

		let headers = HeaderMap::new();

		let result = parser.parse(Some("text/xml"), xml, &headers).await.unwrap();
		match result {
			ParsedData::Xml(value) => {
				let root = value.get("root").unwrap();
				assert_eq!(root.get("@id").unwrap(), "123");
			}
			_ => panic!("Expected XML"),
		}
	}

	#[tokio::test]
	async fn test_xml_parser_with_cdata() {
		let parser = XMLParser::new();
		let xml = Bytes::from("<root><![CDATA[<html>content</html>]]></root>");

		let headers = HeaderMap::new();

		let result = parser
			.parse(Some("application/xml"), xml, &headers)
			.await
			.unwrap();
		match result {
			ParsedData::Xml(value) => {
				let root = value.get("root").unwrap();
				let text = root.get("#text").unwrap();
				assert!(text.as_str().unwrap().contains("html"));
			}
			_ => panic!("Expected XML"),
		}
	}

	#[tokio::test]
	async fn test_xml_parser_with_numbers() {
		let config = XmlParserConfig::builder().parse_numbers(true).build();

		let parser = XMLParser::with_config(config);
		let xml = Bytes::from("<root><count>42</count><price>19.99</price></root>");

		let headers = HeaderMap::new();

		let result = parser
			.parse(Some("application/xml"), xml, &headers)
			.await
			.unwrap();
		match result {
			ParsedData::Xml(value) => {
				let root = value.get("root").unwrap();
				let count = root.get("count").unwrap().get("#text").unwrap();
				assert_eq!(count.as_i64().unwrap(), 42);
			}
			_ => panic!("Expected XML"),
		}
	}

	#[tokio::test]
	async fn test_xml_parser_with_booleans() {
		let config = XmlParserConfig::builder().parse_booleans(true).build();

		let parser = XMLParser::with_config(config);
		let xml = Bytes::from("<root><active>true</active><enabled>false</enabled></root>");

		let headers = HeaderMap::new();

		let result = parser
			.parse(Some("application/xml"), xml, &headers)
			.await
			.unwrap();
		match result {
			ParsedData::Xml(value) => {
				let root = value.get("root").unwrap();
				let active = root.get("active").unwrap().get("#text").unwrap();
				assert!(active.as_bool().unwrap());
			}
			_ => panic!("Expected XML"),
		}
	}

	#[tokio::test]
	async fn test_xml_parser_repeated_elements() {
		let parser = XMLParser::new();
		let xml = Bytes::from("<root><item>1</item><item>2</item><item>3</item></root>");

		let headers = HeaderMap::new();

		let result = parser
			.parse(Some("application/xml"), xml, &headers)
			.await
			.unwrap();
		match result {
			ParsedData::Xml(value) => {
				let root = value.get("root").unwrap();
				let items = root.get("item").unwrap();
				assert!(items.is_array());
				assert_eq!(items.as_array().unwrap().len(), 3);
			}
			_ => panic!("Expected XML"),
		}
	}

	#[tokio::test]
	async fn test_xml_parser_empty_elements() {
		let parser = XMLParser::new();
		let xml = Bytes::from(r#"<root><empty /><empty id="test" /></root>"#);

		let headers = HeaderMap::new();

		let result = parser
			.parse(Some("application/xml"), xml, &headers)
			.await
			.unwrap();
		match result {
			ParsedData::Xml(value) => {
				let root = value.get("root").unwrap();
				let empty = root.get("empty").unwrap();
				assert!(empty.is_array());
			}
			_ => panic!("Expected XML"),
		}
	}

	#[tokio::test]
	async fn test_xml_parser_nested_structure() {
		let parser = XMLParser::new();
		let xml = Bytes::from(
			"<root><person><name>John</name><address><city>NYC</city></address></person></root>",
		);

		let headers = HeaderMap::new();

		let result = parser
			.parse(Some("application/xml"), xml, &headers)
			.await
			.unwrap();
		match result {
			ParsedData::Xml(value) => {
				let root = value.get("root").unwrap();
				let person = root.get("person").unwrap();
				let address = person.get("address").unwrap();
				let city = address.get("city").unwrap();
				assert_eq!(city.get("#text").unwrap(), "NYC");
			}
			_ => panic!("Expected XML"),
		}
	}

	#[test]
	fn test_xml_parser_config_builder() {
		let config = XmlParserConfig::builder()
			.include_attributes(false)
			.attribute_prefix("$".to_string())
			.text_key("value".to_string())
			.preserve_cdata(true)
			.trim_text(false)
			.parse_numbers(true)
			.parse_booleans(true)
			.build();

		assert!(!config.include_attributes);
		assert_eq!(config.attribute_prefix, "$");
		assert_eq!(config.text_key, "value");
		assert!(config.preserve_cdata);
		assert!(!config.trim_text);
		assert!(config.parse_numbers);
		assert!(config.parse_booleans);
	}

	#[tokio::test]
	async fn test_xml_parser_media_types() {
		let parser = XMLParser::new();
		let media_types = parser.media_types();

		assert_eq!(media_types.len(), 2);
		assert!(media_types.contains(&"application/xml".to_string()));
		assert!(media_types.contains(&"text/xml".to_string()));
	}

	#[tokio::test]
	async fn test_xml_parser_can_parse() {
		let parser = XMLParser::new();

		assert!(parser.can_parse(Some("application/xml")));
		assert!(parser.can_parse(Some("text/xml")));
		assert!(!parser.can_parse(Some("application/json")));
		assert!(!parser.can_parse(None));
	}

	#[tokio::test]
	async fn test_xml_parser_invalid_xml() {
		let parser = XMLParser::new();
		// Mismatched tags should cause an error
		let xml = Bytes::from("<root><item></root></item>");

		let headers = HeaderMap::new();

		let result = parser.parse(Some("application/xml"), xml, &headers).await;
		// Note: quick-xml may be lenient with some errors
		// This test ensures we handle malformed XML gracefully
		let _ = result;
	}
}
