//! Text manipulation utilities
/// Capitalize first letter of each word
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::text::capfirst;
///
/// assert_eq!(capfirst("hello world"), "Hello World");
/// assert_eq!(capfirst("test"), "Test");
/// assert_eq!(capfirst("HELLO"), "HELLO");
/// ```
pub fn capfirst(text: &str) -> String {
	let mut result = String::with_capacity(text.len());
	let mut capitalize_next = true;

	for ch in text.chars() {
		if capitalize_next && ch.is_alphabetic() {
			result.extend(ch.to_uppercase());
			capitalize_next = false;
		} else {
			result.push(ch);
			if ch.is_whitespace() {
				capitalize_next = true;
			}
		}
	}
	result
}
/// Convert to title case
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::text::title;
///
/// assert_eq!(title("hello world"), "Hello World");
/// assert_eq!(title("HELLO WORLD"), "Hello World");
/// assert_eq!(title("test case"), "Test Case");
/// ```
pub fn title(text: &str) -> String {
	text.split_whitespace()
		.map(|word| {
			let mut chars = word.chars();
			match chars.next() {
				None => String::new(),
				Some(first) => {
					first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
				}
			}
		})
		.collect::<Vec<_>>()
		.join(" ")
}
/// Pluralize a word based on count
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::text::pluralize;
///
/// assert_eq!(pluralize(1, "apple", None), "apple");
/// assert_eq!(pluralize(2, "apple", None), "apples");
/// assert_eq!(pluralize(0, "apple", None), "apples");
/// assert_eq!(pluralize(2, "person", Some("people")), "people");
/// assert_eq!(pluralize(1, "person", Some("people")), "person");
/// ```
pub fn pluralize(count: i64, singular: &str, plural: Option<&str>) -> String {
	if count == 1 {
		singular.to_string()
	} else {
		plural.unwrap_or(&format!("{}s", singular)).to_string()
	}
}
/// Get ordinal suffix for a number (1st, 2nd, 3rd, etc)
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::text::ordinal;
///
/// assert_eq!(ordinal(1), "1st");
/// assert_eq!(ordinal(2), "2nd");
/// assert_eq!(ordinal(3), "3rd");
/// assert_eq!(ordinal(4), "4th");
/// assert_eq!(ordinal(11), "11th");
/// assert_eq!(ordinal(21), "21st");
/// ```
pub fn ordinal(n: i64) -> String {
	let abs_n = n.unsigned_abs();
	let suffix = match (abs_n % 10, abs_n % 100) {
		(1, 11) => "th",
		(1, _) => "st",
		(2, 12) => "th",
		(2, _) => "nd",
		(3, 13) => "th",
		(3, _) => "rd",
		_ => "th",
	};
	format!("{}{}", n, suffix)
}
/// Add commas to number for readability
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::text::intcomma;
///
/// assert_eq!(intcomma(1000), "1,000");
/// assert_eq!(intcomma(1000000), "1,000,000");
/// assert_eq!(intcomma(123), "123");
/// assert_eq!(intcomma(-1000), "-1,000");
/// ```
pub fn intcomma(n: i64) -> String {
	let s = n.to_string();
	let negative = s.starts_with('-');
	let digits: String = if negative {
		s.chars().skip(1).collect()
	} else {
		s.clone()
	};

	let mut result = String::new();
	for (i, ch) in digits.chars().rev().enumerate() {
		if i > 0 && i % 3 == 0 {
			result.push(',');
		}
		result.push(ch);
	}

	let formatted: String = result.chars().rev().collect();
	if negative {
		format!("-{}", formatted)
	} else {
		formatted
	}
}
/// Add commas to float
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::text::floatcomma;
///
/// assert_eq!(floatcomma(1000.5, 2), "1,000.50");
/// assert_eq!(floatcomma(1234567.89, 2), "1,234,567.89");
/// assert_eq!(floatcomma(0.0, 2), "0.00");
/// ```
pub fn floatcomma(n: f64, decimals: usize) -> String {
	let formatted = format!("{:.prec$}", n, prec = decimals);
	let parts: Vec<&str> = formatted.split('.').collect();

	let integer_part = if let Some(int_str) = parts.first() {
		if let Ok(int_val) = int_str.parse::<i64>() {
			intcomma(int_val)
		} else {
			int_str.to_string()
		}
	} else {
		"0".to_string()
	};

	if let Some(decimal_part) = parts.get(1) {
		format!("{}.{}", integer_part, decimal_part)
	} else {
		integer_part
	}
}
/// Left pad string to specified width
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::text::ljust;
///
/// assert_eq!(ljust("test", 10, ' '), "test      ");
/// assert_eq!(ljust("test", 10, '-'), "test------");
/// assert_eq!(ljust("test", 3, ' '), "test");
/// ```
pub fn ljust(text: &str, width: usize, fillchar: char) -> String {
	let current_len = text.chars().count();
	if current_len >= width {
		text.to_string()
	} else {
		let padding = fillchar.to_string().repeat(width - current_len);
		format!("{}{}", text, padding)
	}
}
/// Right pad string to specified width
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::text::rjust;
///
/// assert_eq!(rjust("test", 10, ' '), "      test");
/// assert_eq!(rjust("test", 10, '-'), "------test");
/// assert_eq!(rjust("test", 3, ' '), "test");
/// ```
pub fn rjust(text: &str, width: usize, fillchar: char) -> String {
	let current_len = text.chars().count();
	if current_len >= width {
		text.to_string()
	} else {
		let padding = fillchar.to_string().repeat(width - current_len);
		format!("{}{}", padding, text)
	}
}
/// Center string in specified width
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::text::center;
///
/// assert_eq!(center("test", 10, ' '), "   test   ");
/// assert_eq!(center("test", 10, '-'), "---test---");
/// assert_eq!(center("test", 3, ' '), "test");
/// ```
pub fn center(text: &str, width: usize, fillchar: char) -> String {
	let current_len = text.chars().count();
	if current_len >= width {
		return text.to_string();
	}

	let total_padding = width - current_len;
	let left_padding = total_padding / 2;
	let right_padding = total_padding - left_padding;

	format!(
		"{}{}{}",
		fillchar.to_string().repeat(left_padding),
		text,
		fillchar.to_string().repeat(right_padding)
	)
}
/// Phone number formatter
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::text::phone_format;
///
/// assert_eq!(phone_format("1234567890"), "(123) 456-7890");
/// assert_eq!(phone_format("11234567890"), "+1 (123) 456-7890");
/// assert_eq!(phone_format("(123) 456-7890"), "(123) 456-7890");
/// assert_eq!(phone_format("123"), "123");
/// ```
pub fn phone_format(number: &str) -> String {
	let digits: String = number.chars().filter(|c| c.is_ascii_digit()).collect();

	match digits.len() {
		10 => format!("({}) {}-{}", &digits[0..3], &digits[3..6], &digits[6..10]),
		11 if digits.starts_with('1') => format!(
			"+1 ({}) {}-{}",
			&digits[1..4],
			&digits[4..7],
			&digits[7..11]
		),
		_ => number.to_string(),
	}
}

/// Convert snake_case field names to human-readable labels
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::text::humanize_field_name;
///
/// assert_eq!(humanize_field_name("created_at"), "Created At");
/// assert_eq!(humanize_field_name("user_id"), "User Id");
/// assert_eq!(humanize_field_name("is_active"), "Is Active");
/// ```
pub fn humanize_field_name(field_name: &str) -> String {
	let with_spaces = field_name.replace('_', " ");
	title(&with_spaces)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_capfirst() {
		assert_eq!(capfirst("hello world"), "Hello World");
		assert_eq!(capfirst("HELLO"), "HELLO");
		assert_eq!(capfirst("test"), "Test");
	}

	#[test]
	fn test_title() {
		assert_eq!(title("hello world"), "Hello World");
		assert_eq!(title("HELLO WORLD"), "Hello World");
		assert_eq!(title("test case"), "Test Case");
	}

	#[test]
	fn test_utils_text_pluralize() {
		assert_eq!(pluralize(1, "apple", None), "apple");
		assert_eq!(pluralize(2, "apple", None), "apples");
		assert_eq!(pluralize(0, "apple", None), "apples");
		assert_eq!(pluralize(2, "person", Some("people")), "people");
		assert_eq!(pluralize(1, "person", Some("people")), "person");
	}

	#[test]
	fn test_utils_text_ordinal() {
		assert_eq!(ordinal(1), "1st");
		assert_eq!(ordinal(2), "2nd");
		assert_eq!(ordinal(3), "3rd");
		assert_eq!(ordinal(4), "4th");
		assert_eq!(ordinal(11), "11th");
		assert_eq!(ordinal(21), "21st");
		assert_eq!(ordinal(22), "22nd");
		assert_eq!(ordinal(23), "23rd");
	}

	#[test]
	fn test_utils_text_intcomma() {
		assert_eq!(intcomma(1000), "1,000");
		assert_eq!(intcomma(1000000), "1,000,000");
		assert_eq!(intcomma(123), "123");
		assert_eq!(intcomma(-1000), "-1,000");
	}

	#[test]
	fn test_floatcomma() {
		assert_eq!(floatcomma(1000.5, 2), "1,000.50");
		assert_eq!(floatcomma(1234567.89, 2), "1,234,567.89");
	}

	#[test]
	fn test_ljust() {
		assert_eq!(ljust("test", 10, ' '), "test      ");
		assert_eq!(ljust("test", 10, '-'), "test------");
		assert_eq!(ljust("test", 3, ' '), "test");
	}

	#[test]
	fn test_rjust() {
		assert_eq!(rjust("test", 10, ' '), "      test");
		assert_eq!(rjust("test", 10, '-'), "------test");
		assert_eq!(rjust("test", 3, ' '), "test");
	}

	#[test]
	fn test_center() {
		assert_eq!(center("test", 10, ' '), "   test   ");
		assert_eq!(center("test", 10, '-'), "---test---");
		assert_eq!(center("test", 3, ' '), "test");
	}

	#[test]
	fn test_phone_format() {
		assert_eq!(phone_format("1234567890"), "(123) 456-7890");
		assert_eq!(phone_format("11234567890"), "+1 (123) 456-7890");
		assert_eq!(phone_format("(123) 456-7890"), "(123) 456-7890");
		assert_eq!(phone_format("123"), "123");
	}

	#[test]
	fn test_capfirst_empty() {
		assert_eq!(capfirst(""), "");
	}

	#[test]
	fn test_capfirst_unicode() {
		assert_eq!(capfirst("こんにちは 世界"), "こんにちは 世界");
	}

	#[test]
	fn test_title_empty() {
		assert_eq!(title(""), "");
	}

	#[test]
	fn test_title_single_word() {
		assert_eq!(title("hello"), "Hello");
	}

	#[test]
	fn test_pluralize_negative() {
		assert_eq!(pluralize(-1, "apple", None), "apples");
		assert_eq!(pluralize(-2, "apple", None), "apples");
	}

	#[test]
	fn test_pluralize_zero() {
		assert_eq!(pluralize(0, "item", None), "items");
	}

	#[test]
	fn test_ordinal_teens() {
		assert_eq!(ordinal(11), "11th");
		assert_eq!(ordinal(12), "12th");
		assert_eq!(ordinal(13), "13th");
	}

	#[test]
	fn test_ordinal_hundreds() {
		assert_eq!(ordinal(101), "101st");
		assert_eq!(ordinal(102), "102nd");
		assert_eq!(ordinal(103), "103rd");
		assert_eq!(ordinal(111), "111th");
	}

	#[test]
	fn test_intcomma_zero() {
		assert_eq!(intcomma(0), "0");
	}

	#[test]
	fn test_intcomma_large_numbers() {
		assert_eq!(intcomma(1_000_000_000), "1,000,000,000");
		assert_eq!(intcomma(-1_000_000_000), "-1,000,000,000");
	}

	#[test]
	fn test_floatcomma_zero() {
		assert_eq!(floatcomma(0.0, 2), "0.00");
	}

	#[test]
	fn test_floatcomma_negative() {
		assert_eq!(floatcomma(-1234.56, 2), "-1,234.56");
	}

	#[test]
	fn test_floatcomma_no_decimals() {
		assert_eq!(floatcomma(1234.0, 0), "1,234");
	}

	#[test]
	fn test_ljust_exact_width() {
		assert_eq!(ljust("test", 4, ' '), "test");
	}

	#[test]
	fn test_ljust_unicode() {
		assert_eq!(ljust("こんにちは", 10, ' '), "こんにちは     ");
	}

	#[test]
	fn test_rjust_exact_width() {
		assert_eq!(rjust("test", 4, ' '), "test");
	}

	#[test]
	fn test_rjust_unicode() {
		assert_eq!(rjust("こんにちは", 10, ' '), "     こんにちは");
	}

	#[test]
	fn test_center_exact_width() {
		assert_eq!(center("test", 4, ' '), "test");
	}

	#[test]
	fn test_center_odd_padding() {
		assert_eq!(center("test", 9, ' '), "  test   ");
	}

	#[test]
	fn test_center_unicode() {
		assert_eq!(center("こんにちは", 10, '-'), "--こんにちは---");
	}

	#[test]
	fn test_phone_format_empty() {
		assert_eq!(phone_format(""), "");
	}

	#[test]
	fn test_phone_format_with_dashes() {
		assert_eq!(phone_format("123-456-7890"), "(123) 456-7890");
	}

	#[test]
	fn test_ordinal_negative() {
		// Negative numbers should use the same suffix as their absolute value
		assert_eq!(ordinal(-1), "-1st");
		assert_eq!(ordinal(-2), "-2nd");
		assert_eq!(ordinal(-3), "-3rd");
		assert_eq!(ordinal(-4), "-4th");
		assert_eq!(ordinal(-11), "-11th");
		assert_eq!(ordinal(-12), "-12th");
		assert_eq!(ordinal(-13), "-13th");
		assert_eq!(ordinal(-21), "-21st");
	}

	#[test]
	fn test_capfirst_numbers() {
		assert_eq!(capfirst("123 test"), "123 Test");
	}

	#[test]
	fn test_title_mixed_case() {
		assert_eq!(title("hElLo WoRlD"), "Hello World");
	}

	#[test]
	fn test_humanize_field_name() {
		assert_eq!(humanize_field_name("created_at"), "Created At");
		assert_eq!(humanize_field_name("user_id"), "User Id");
		assert_eq!(humanize_field_name("is_active"), "Is Active");
	}

	#[test]
	fn test_humanize_field_name_single_word() {
		assert_eq!(humanize_field_name("name"), "Name");
	}

	#[test]
	fn test_humanize_field_name_no_underscores() {
		assert_eq!(humanize_field_name("username"), "Username");
	}

	#[test]
	fn test_humanize_field_name_multiple_underscores() {
		assert_eq!(humanize_field_name("user_full_name"), "User Full Name");
	}

	#[test]
	fn test_humanize_field_name_empty() {
		assert_eq!(humanize_field_name(""), "");
	}
}

#[cfg(test)]
mod proptests {
	use super::*;
	use proptest::prelude::*;

	proptest! {
		#[test]
		fn prop_pluralize_one(s in "\\w+") {
			assert_eq!(pluralize(1, &s, None), s);
		}

		#[test]
		fn prop_pluralize_not_one(n in 2i64..100, s in "\\w+") {
			let result = pluralize(n, &s, None);
			assert_ne!(result, s);
		}

		#[test]
		fn prop_ordinal_format(n in 1i64..1000) {
			let ord = ordinal(n);
			assert!(ord.starts_with(&n.to_string()));
			assert!(ord.ends_with("st") || ord.ends_with("nd") || ord.ends_with("rd") || ord.ends_with("th"));
		}

		#[test]
		fn prop_ljust_min_length(s in "\\w+", width in 5usize..50) {
			let padded = ljust(&s, width, ' ');
			assert!(padded.chars().count() >= s.chars().count());
		}

		#[test]
		fn prop_rjust_min_length(s in "\\w+", width in 5usize..50) {
			let padded = rjust(&s, width, ' ');
			assert!(padded.chars().count() >= s.chars().count());
		}

		#[test]
		fn prop_center_min_length(s in "\\w+", width in 5usize..50) {
			let centered = center(&s, width, ' ');
			assert!(centered.chars().count() >= s.chars().count());
		}
	}
}
