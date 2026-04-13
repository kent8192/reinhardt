/// Convert a snake_case identifier to PascalCase and append a suffix.
///
/// # Examples
///
/// - `to_pascal_case_with_suffix("users", "Urls")` → `"UsersUrls"`
/// - `to_pascal_case_with_suffix("blog_posts", "Urls")` → `"BlogPostsUrls"`
/// - `to_pascal_case_with_suffix("auth_v2", "Urls")` → `"AuthV2Urls"`
// allow(dead_code): Available for proc-macro-level PascalCase conversion
// where the paste crate cannot be used.
#[allow(dead_code)]
pub(crate) fn to_pascal_case_with_suffix(name: &str, suffix: &str) -> String {
	let mut result = String::new();
	for segment in name.split('_') {
		let mut chars = segment.chars();
		if let Some(first) = chars.next() {
			result.push(first.to_ascii_uppercase());
			result.extend(chars);
		}
	}
	result.push_str(suffix);
	result
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn simple_name() {
		assert_eq!(to_pascal_case_with_suffix("users", "Urls"), "UsersUrls");
	}

	#[test]
	fn snake_case_name() {
		assert_eq!(
			to_pascal_case_with_suffix("blog_posts", "Urls"),
			"BlogPostsUrls"
		);
	}

	#[test]
	fn short_name() {
		assert_eq!(to_pascal_case_with_suffix("a", "Urls"), "AUrls");
	}

	#[test]
	fn name_with_digits() {
		assert_eq!(to_pascal_case_with_suffix("auth_v2", "Urls"), "AuthV2Urls");
	}

	#[test]
	fn single_segment() {
		assert_eq!(to_pascal_case_with_suffix("api", "Urls"), "ApiUrls");
	}

	#[test]
	fn long_name() {
		assert_eq!(
			to_pascal_case_with_suffix("my_long_app_name", "Urls"),
			"MyLongAppNameUrls"
		);
	}
}
