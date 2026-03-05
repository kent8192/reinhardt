//! Message catalog for storing translations

use std::collections::HashMap;

/// A message catalog containing translations for a specific locale
///
/// # Example
/// ```
/// use reinhardt_i18n::MessageCatalog;
///
/// let mut catalog = MessageCatalog::new("fr");
/// catalog.add_translation("Hello", "Bonjour");
/// catalog.add_plural_str("item", "items", vec!["article", "articles"]);
///
/// assert_eq!(catalog.get("Hello"), Some(&"Bonjour".to_string()));
/// assert_eq!(catalog.get_plural("item", 1), Some(&"article".to_string()));
/// assert_eq!(catalog.get_plural("item", 5), Some(&"articles".to_string()));
/// ```
#[derive(Debug, Clone)]
pub struct MessageCatalog {
	locale: String,
	messages: HashMap<String, String>,
	plurals: HashMap<String, Vec<String>>,
	contexts: HashMap<(String, String), String>,
	context_plurals: HashMap<(String, String), Vec<String>>,
}

impl MessageCatalog {
	/// Create a new message catalog for the given locale
	pub fn new(locale: &str) -> Self {
		Self {
			locale: locale.to_string(),
			messages: HashMap::new(),
			plurals: HashMap::new(),
			contexts: HashMap::new(),
			context_plurals: HashMap::new(),
		}
	}

	/// Get the locale for this catalog
	pub fn locale(&self) -> &str {
		&self.locale
	}

	/// Add a simple translation
	pub fn add_translation(&mut self, message: impl Into<String>, translation: impl Into<String>) {
		self.messages.insert(message.into(), translation.into());
	}

	/// Add a simple translation (alias for add_translation)
	pub fn add(&mut self, message: impl Into<String>, translation: impl Into<String>) {
		self.messages.insert(message.into(), translation.into());
	}

	/// Add a plural translation with `Vec<String>`
	///
	/// The singular key is used as-is without any implicit parsing.
	/// For context-qualified plurals, use `add_context_plural()` instead.
	pub fn add_plural(&mut self, singular: impl Into<String>, forms: Vec<String>) {
		self.plurals.insert(singular.into(), forms);
	}

	/// Add a plural translation with string slices
	pub fn add_plural_str(
		&mut self,
		singular: impl Into<String>,
		_plural: impl Into<String>,
		forms: Vec<&str>,
	) {
		self.plurals.insert(
			singular.into(),
			forms.iter().map(|s| s.to_string()).collect(),
		);
	}

	/// Add a contextual translation
	pub fn add_context(
		&mut self,
		context: impl Into<String>,
		message: impl Into<String>,
		translation: impl Into<String>,
	) {
		self.contexts
			.insert((context.into(), message.into()), translation.into());
	}

	/// Add a contextual translation with string slices
	pub fn add_context_str(
		&mut self,
		context: impl Into<String>,
		message: impl Into<String>,
		translation: impl Into<String>,
	) {
		self.contexts
			.insert((context.into(), message.into()), translation.into());
	}

	/// Add a contextual plural translation
	pub fn add_context_plural(
		&mut self,
		context: impl Into<String>,
		singular: impl Into<String>,
		_plural: impl Into<String>,
		forms: Vec<&str>,
	) {
		self.context_plurals.insert(
			(context.into(), singular.into()),
			forms.iter().map(|s| s.to_string()).collect(),
		);
	}

	/// Get a translation
	pub fn get(&self, message: &str) -> Option<&String> {
		self.messages.get(message)
	}

	/// Get a plural translation
	pub fn get_plural(&self, singular: &str, count: usize) -> Option<&String> {
		let forms = self.plurals.get(singular)?;
		let index = self.plural_form(count);
		forms.get(index)
	}

	/// Get a contextual translation
	pub fn get_context(&self, context: &str, message: &str) -> Option<&String> {
		self.contexts
			.get(&(context.to_string(), message.to_string()))
	}

	/// Get a contextual plural translation
	pub fn get_context_plural(
		&self,
		context: &str,
		singular: &str,
		count: usize,
	) -> Option<&String> {
		let forms = self
			.context_plurals
			.get(&(context.to_string(), singular.to_string()))?;
		let index = self.plural_form(count);
		forms.get(index)
	}

	/// Determine the plural form index for a given count
	///
	/// Uses language-specific plural rules based on locale. Supports:
	/// - East Asian (ja, zh, ko, vi, th, id, ms): single form (index 0)
	/// - Romance/Brazilian (fr, pt_BR): 0 and 1 singular, 2+ plural
	/// - Slavic 3-form (ru, uk, be, sr, hr, bs): special mod-based rules
	/// - Polish (pl): 3-form with distinct rules
	/// - Czech/Slovak (cs, sk): 3-form with distinct rules
	/// - Slovenian (sl): 4-form with dual support
	/// - Arabic (ar): 6-form rules
	/// - Celtic (ga, cy): various multi-form rules
	/// - Germanic/default (en, de, nl, etc.): 1 singular, rest plural
	fn plural_form(&self, count: usize) -> usize {
		let lang = self.locale.split(['-', '_']).next().unwrap_or(&self.locale);

		match lang {
			// East Asian and others with no plural forms (single form)
			"ja" | "zh" | "ko" | "vi" | "th" | "id" | "ms" | "tr" | "fa" | "ka" => 0,

			// French: 0 and 1 are singular
			"fr" => {
				if count == 0 || count == 1 {
					0
				} else {
					1
				}
			}

			// Portuguese: Brazilian Portuguese uses French-style rules (0,1 singular)
			// European Portuguese uses Germanic-style rules (1 singular)
			"pt" => {
				if self.locale.starts_with("pt_BR") || self.locale.starts_with("pt-BR") {
					if count == 0 || count == 1 { 0 } else { 1 }
				} else if count == 1 {
					0
				} else {
					1
				}
			}

			// Russian, Ukrainian, Belarusian, Serbian, Croatian, Bosnian (3 forms)
			// form 0: n%10==1 && n%100!=11
			// form 1: n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20)
			// form 2: everything else
			"ru" | "uk" | "be" | "sr" | "hr" | "bs" => {
				let n100 = count % 100;
				let n10 = count % 10;
				if n10 == 1 && n100 != 11 {
					0
				} else if (2..=4).contains(&n10) && !(10..20).contains(&n100) {
					1
				} else {
					2
				}
			}

			// Polish (3 forms)
			// form 0: n==1
			// form 1: n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20)
			// form 2: everything else
			"pl" => {
				let n100 = count % 100;
				let n10 = count % 10;
				if count == 1 {
					0
				} else if (2..=4).contains(&n10) && !(10..20).contains(&n100) {
					1
				} else {
					2
				}
			}

			// Czech, Slovak (3 forms)
			// form 0: n==1
			// form 1: n>=2 && n<=4
			// form 2: everything else
			"cs" | "sk" => {
				if count == 1 {
					0
				} else if (2..=4).contains(&count) {
					1
				} else {
					2
				}
			}

			// Slovenian (4 forms)
			// form 0: n%100==1
			// form 1: n%100==2
			// form 2: n%100==3 || n%100==4
			// form 3: everything else
			"sl" => {
				let n100 = count % 100;
				if n100 == 1 {
					0
				} else if n100 == 2 {
					1
				} else if n100 == 3 || n100 == 4 {
					2
				} else {
					3
				}
			}

			// Arabic (6 forms)
			// form 0: n==0
			// form 1: n==1
			// form 2: n==2
			// form 3: n%100>=3 && n%100<=10
			// form 4: n%100>=11
			// form 5: everything else (n>=100 with n%100<3)
			"ar" => {
				let n100 = count % 100;
				if count == 0 {
					0
				} else if count == 1 {
					1
				} else if count == 2 {
					2
				} else if (3..=10).contains(&n100) {
					3
				} else if n100 >= 11 {
					4
				} else {
					5
				}
			}

			// Irish (5 forms)
			"ga" => {
				if count == 1 {
					0
				} else if count == 2 {
					1
				} else if (3..=6).contains(&count) {
					2
				} else if (7..=10).contains(&count) {
					3
				} else {
					4
				}
			}

			// Welsh (6 forms)
			"cy" => match count {
				0 => 0,
				1 => 1,
				2 => 2,
				3 => 3,
				6 => 4,
				_ => 5,
			},

			// Lithuanian (3 forms)
			// form 0: n%10==1 && n%100!=11
			// form 1: n%10>=2 && (n%100<10 || n%100>=20)
			// form 2: everything else
			"lt" => {
				let n100 = count % 100;
				let n10 = count % 10;
				if n10 == 1 && n100 != 11 {
					0
				} else if n10 >= 2 && !(10..20).contains(&n100) {
					1
				} else {
					2
				}
			}

			// Latvian (3 forms)
			// form 0: n%10==1 && n%100!=11
			// form 1: n!=0
			// form 2: n==0
			"lv" => {
				let n100 = count % 100;
				let n10 = count % 10;
				if n10 == 1 && n100 != 11 {
					0
				} else if count != 0 {
					1
				} else {
					2
				}
			}

			// Romanian (3 forms)
			// form 0: n==1
			// form 1: n==0 || (n%100>0 && n%100<20)
			// form 2: everything else
			"ro" => {
				let n100 = count % 100;
				if count == 1 {
					0
				} else if count == 0 || (n100 > 0 && n100 < 20) {
					1
				} else {
					2
				}
			}

			// Default: Germanic-style (en, de, nl, sv, da, nb, nn, etc.)
			// form 0: n==1 (singular)
			// form 1: everything else (plural)
			_ => {
				if count == 1 {
					0
				} else {
					1
				}
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_message_catalog_basic() {
		// Arrange
		let mut catalog = MessageCatalog::new("es");
		catalog.add_translation("Good morning", "Buenos días");

		// Act
		let result = catalog.get("Good morning");
		let missing = catalog.get("Unknown");

		// Assert
		assert_eq!(result, Some(&"Buenos días".to_string()));
		assert_eq!(missing, None);
	}

	#[rstest]
	fn test_message_catalog_plural() {
		// Arrange
		let mut catalog = MessageCatalog::new("fr");
		catalog.add_plural_str("car", "cars", vec!["voiture", "voitures"]);

		// Act
		let singular = catalog.get_plural("car", 1);
		let plural = catalog.get_plural("car", 3);

		// Assert
		assert_eq!(singular, Some(&"voiture".to_string()));
		assert_eq!(plural, Some(&"voitures".to_string()));
	}

	#[rstest]
	fn test_message_catalog_context() {
		// Arrange
		let mut catalog = MessageCatalog::new("de");
		catalog.add_context_str("menu", "File", "Datei");
		catalog.add_context_str("verb", "File", "Ablegen");

		// Act
		let menu_result = catalog.get_context("menu", "File");
		let verb_result = catalog.get_context("verb", "File");

		// Assert
		assert_eq!(menu_result, Some(&"Datei".to_string()));
		assert_eq!(verb_result, Some(&"Ablegen".to_string()));
	}

	#[rstest]
	fn test_add_plural_does_not_split_colon_in_key() {
		// Arrange: key containing colons should NOT be split
		let mut catalog = MessageCatalog::new("en");
		catalog.add_plural(
			"Time: 12:00",
			vec![
				"Time: 12:00 singular".to_string(),
				"Time: 12:00 plural".to_string(),
			],
		);

		// Act: lookup with the exact key should succeed
		let result = catalog.get_plural("Time: 12:00", 1);

		// Assert: the key is stored as-is, not split by colon
		assert_eq!(result, Some(&"Time: 12:00 singular".to_string()));
	}

	#[rstest]
	fn test_add_plural_with_colon_is_reachable_via_get_plural() {
		// Arrange: key with colon stored via add_plural
		let mut catalog = MessageCatalog::new("en");
		catalog.add_plural(
			"Error: file not found",
			vec!["singular form".to_string(), "plural form".to_string()],
		);

		// Act
		let singular = catalog.get_plural("Error: file not found", 1);
		let plural = catalog.get_plural("Error: file not found", 5);

		// Assert: both forms are reachable
		assert_eq!(singular, Some(&"singular form".to_string()));
		assert_eq!(plural, Some(&"plural form".to_string()));
	}

	#[rstest]
	fn test_add_plural_colon_key_not_stored_in_context_plurals() {
		// Arrange: key with colon should NOT end up in context_plurals
		let mut catalog = MessageCatalog::new("en");
		catalog.add_plural(
			"Note: see section 3.2",
			vec!["singular".to_string(), "plural".to_string()],
		);

		// Act: looking up as context should find nothing
		let context_result = catalog.get_context_plural("Note", " see section 3.2", 1);

		// Assert: no entry in context_plurals from implicit splitting
		assert_eq!(context_result, None);
	}

	#[rstest]
	#[case("ru", 1, 0)] // 1 file -> singular
	#[case("ru", 2, 1)] // 2 files -> second form
	#[case("ru", 5, 2)] // 5 files -> third form
	#[case("ru", 11, 2)] // 11 files -> third form (special teen)
	#[case("ru", 21, 0)] // 21 files -> singular
	#[case("ru", 22, 1)] // 22 files -> second form
	#[case("ru", 25, 2)] // 25 files -> third form
	#[case("ru", 111, 2)] // 111 files -> third form (teen in hundreds)
	#[case("ru", 112, 2)] // 112 files -> third form
	#[case("ru", 121, 0)] // 121 files -> singular
	fn test_plural_form_russian(
		#[case] locale: &str,
		#[case] count: usize,
		#[case] expected: usize,
	) {
		// Arrange
		let catalog = MessageCatalog::new(locale);

		// Act
		let result = catalog.plural_form(count);

		// Assert
		assert_eq!(result, expected, "Russian plural form for count={}", count);
	}

	#[rstest]
	#[case("pl", 1, 0)] // singular
	#[case("pl", 2, 1)] // second form
	#[case("pl", 5, 2)] // third form
	#[case("pl", 12, 2)] // teens -> third form
	#[case("pl", 22, 1)] // second form
	#[case("pl", 0, 2)] // zero -> third form
	fn test_plural_form_polish(
		#[case] locale: &str,
		#[case] count: usize,
		#[case] expected: usize,
	) {
		// Arrange
		let catalog = MessageCatalog::new(locale);

		// Act
		let result = catalog.plural_form(count);

		// Assert
		assert_eq!(result, expected, "Polish plural form for count={}", count);
	}

	#[rstest]
	#[case("ar", 0, 0)] // zero form
	#[case("ar", 1, 1)] // singular
	#[case("ar", 2, 2)] // dual
	#[case("ar", 3, 3)] // few (3-10)
	#[case("ar", 10, 3)] // few (3-10)
	#[case("ar", 11, 4)] // many (11-99)
	#[case("ar", 99, 4)] // many (11-99)
	#[case("ar", 100, 5)] // other
	fn test_plural_form_arabic(
		#[case] locale: &str,
		#[case] count: usize,
		#[case] expected: usize,
	) {
		// Arrange
		let catalog = MessageCatalog::new(locale);

		// Act
		let result = catalog.plural_form(count);

		// Assert
		assert_eq!(result, expected, "Arabic plural form for count={}", count);
	}

	#[rstest]
	#[case("cs", 1, 0)] // singular
	#[case("cs", 2, 1)] // few
	#[case("cs", 4, 1)] // few
	#[case("cs", 5, 2)] // other
	#[case("cs", 0, 2)] // other
	fn test_plural_form_czech(#[case] locale: &str, #[case] count: usize, #[case] expected: usize) {
		// Arrange
		let catalog = MessageCatalog::new(locale);

		// Act
		let result = catalog.plural_form(count);

		// Assert
		assert_eq!(result, expected, "Czech plural form for count={}", count);
	}

	#[rstest]
	#[case("sl", 1, 0)] // n%100==1
	#[case("sl", 101, 0)] // n%100==1
	#[case("sl", 2, 1)] // n%100==2
	#[case("sl", 102, 1)] // n%100==2
	#[case("sl", 3, 2)] // n%100==3 or 4
	#[case("sl", 4, 2)] // n%100==3 or 4
	#[case("sl", 5, 3)] // everything else
	fn test_plural_form_slovenian(
		#[case] locale: &str,
		#[case] count: usize,
		#[case] expected: usize,
	) {
		// Arrange
		let catalog = MessageCatalog::new(locale);

		// Act
		let result = catalog.plural_form(count);

		// Assert
		assert_eq!(
			result, expected,
			"Slovenian plural form for count={}",
			count
		);
	}

	#[rstest]
	#[case("ja", 1, 0)]
	#[case("ja", 5, 0)]
	#[case("zh", 100, 0)]
	#[case("ko", 0, 0)]
	#[case("vi", 42, 0)]
	fn test_plural_form_east_asian_always_zero(
		#[case] locale: &str,
		#[case] count: usize,
		#[case] expected: usize,
	) {
		// Arrange
		let catalog = MessageCatalog::new(locale);

		// Act
		let result = catalog.plural_form(count);

		// Assert
		assert_eq!(
			result, expected,
			"East Asian plural form for {}:{}",
			locale, count
		);
	}

	#[rstest]
	#[case("fr", 0, 0)] // French: 0 is singular
	#[case("fr", 1, 0)] // French: 1 is singular
	#[case("fr", 2, 1)] // French: 2+ is plural
	fn test_plural_form_french(
		#[case] locale: &str,
		#[case] count: usize,
		#[case] expected: usize,
	) {
		// Arrange
		let catalog = MessageCatalog::new(locale);

		// Act
		let result = catalog.plural_form(count);

		// Assert
		assert_eq!(result, expected, "French plural form for count={}", count);
	}

	#[rstest]
	#[case("en", 1, 0)] // English: 1 is singular
	#[case("en", 0, 1)] // English: 0 is plural
	#[case("en", 2, 1)] // English: 2+ is plural
	#[case("de", 1, 0)] // German: same as English
	#[case("de", 2, 1)]
	fn test_plural_form_germanic_default(
		#[case] locale: &str,
		#[case] count: usize,
		#[case] expected: usize,
	) {
		// Arrange
		let catalog = MessageCatalog::new(locale);

		// Act
		let result = catalog.plural_form(count);

		// Assert
		assert_eq!(
			result, expected,
			"Germanic plural form for {}:{}",
			locale, count
		);
	}

	#[rstest]
	fn test_plural_form_locale_with_region_code() {
		// Arrange: locale with region code should extract the language part
		let catalog = MessageCatalog::new("ru-RU");

		// Act
		let form_1 = catalog.plural_form(1);
		let form_2 = catalog.plural_form(2);
		let form_5 = catalog.plural_form(5);

		// Assert
		assert_eq!(form_1, 0, "ru-RU: 1 should be singular");
		assert_eq!(form_2, 1, "ru-RU: 2 should be second form");
		assert_eq!(form_5, 2, "ru-RU: 5 should be third form");
	}
}
