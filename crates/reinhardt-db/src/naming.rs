//! Identifier-naming helpers shared by the migration autodetector and the
//! ORM runtime.
//!
//! These functions are intentionally feature-flag-agnostic so that the
//! `orm` feature (which composes through-table names at runtime) and the
//! `migrations` feature (which composes the same names at autodetect time)
//! see exactly the same canonical rule. Keeping a single implementation
//! here prevents the runtime/migration divergence that #4659 surfaced.

/// Convert an identifier to `snake_case`.
///
/// Handles multiple separators (`_`, `.`, `-`, space) and camelCase
/// boundaries, including acronym runs (`HTTPRequest` -> `http_request`).
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_db::naming::to_snake_case;
///
/// assert_eq!(to_snake_case("BlogPost"), "blog_post");
/// assert_eq!(to_snake_case("HTTPRequest"), "http_request");
/// assert_eq!(to_snake_case("public.users"), "public_users");
/// ```
pub fn to_snake_case(name: &str) -> String {
	if name.is_empty() {
		return String::new();
	}

	let mut result = String::with_capacity(name.len() + 4);
	let chars: Vec<char> = name.chars().collect();
	let mut prev_was_separator = true; // Treat start as separator to avoid leading underscore

	for i in 0..chars.len() {
		let ch = chars[i];

		// Handle separators: _, -, space, .
		if ch == '_' || ch == '-' || ch == ' ' || ch == '.' {
			// Only add underscore if previous char was not a separator
			if !prev_was_separator && !result.is_empty() {
				result.push('_');
			}
			prev_was_separator = true;
		} else if ch.is_ascii_uppercase() {
			if !prev_was_separator && i > 0 {
				let prev = chars[i - 1];
				let next = chars.get(i + 1);

				// Add underscore if:
				// 1. Previous char is lowercase (normal camelCase boundary)
				// OR
				// 2. Previous char is uppercase AND next char exists AND is lowercase
				//    (this handles acronyms like HTTPRequest -> http_request)
				if prev.is_ascii_lowercase()
					|| (prev.is_ascii_uppercase() && next.is_some_and(|&n| n.is_ascii_lowercase()))
				{
					result.push('_');
				}
			}
			result.push(ch.to_ascii_lowercase());
			prev_was_separator = false;
		} else {
			result.push(ch.to_ascii_lowercase());
			prev_was_separator = false;
		}
	}

	result
}

/// Truncate a database identifier to PostgreSQL's 63-byte limit with a stable hash suffix.
#[doc(hidden)]
pub fn truncate_identifier_with_hash(logical_name: &str) -> String {
	const MAX_IDENTIFIER_LENGTH: usize = 63;
	const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
	const FNV_PRIME: u64 = 0x00000100000001b3;
	if logical_name.len() <= MAX_IDENTIFIER_LENGTH {
		return logical_name.to_string();
	}

	let hash = logical_name
		.as_bytes()
		.iter()
		.fold(FNV_OFFSET_BASIS, |hash, byte| {
			(hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
		});
	let hash = format!("{hash:016x}");
	let prefix_len = MAX_IDENTIFIER_LENGTH - hash.len() - 1;
	let boundary = logical_name
		.char_indices()
		.map(|(index, _)| index)
		.take_while(|index| *index <= prefix_len)
		.last()
		.unwrap_or(0);
	format!("{}_{}", &logical_name[..boundary], hash)
}

#[doc(hidden)]
pub fn generated_unique_constraint_names(
	table: &str,
	fields: &[String],
	reserved: &[String],
) -> Vec<(String, String)> {
	let mut fields = fields.to_vec();
	fields.sort_unstable();
	let mut generated = Vec::with_capacity(fields.len());

	for field in fields {
		let tuple_digest = stable_constraint_name_hash(&format!("{table}\0{field}"));
		let base_name = bounded_constraint_identifier(&format!(
			"{}_{}_uniq_{tuple_digest:08x}",
			safe_constraint_table_fragment(table),
			safe_constraint_name_fragment(&field)
		));
		let is_taken = |candidate: &str| {
			reserved
				.iter()
				.chain(generated.iter().map(|(name, _)| name))
				.any(|name| name.eq_ignore_ascii_case(candidate))
		};
		let name = if !is_taken(&base_name) {
			base_name
		} else {
			let field_digest = stable_constraint_name_hash(&field);
			let mut candidate =
				bounded_constraint_identifier(&format!("{base_name}_field_{field_digest:08x}"));
			let mut suffix = 2;
			while is_taken(&candidate) {
				candidate = bounded_constraint_identifier(&format!(
					"{base_name}_field_{field_digest:08x}_{suffix}"
				));
				suffix += 1;
			}
			candidate
		};
		generated.push((name, field));
	}

	generated
}

#[doc(hidden)]
pub fn foreign_key_constraint_name(table: &str, column: &str) -> String {
	format!("fk_{table}_{column}")
}

#[doc(hidden)]
pub fn enum_domain_constraint_name(table: &str, column: &str) -> String {
	truncate_identifier_with_hash(&format!("{table}_{column}_model_enum_check"))
}

fn safe_constraint_name_fragment(value: &str) -> String {
	let mut fragment = String::with_capacity(value.len());
	for character in value.chars() {
		if character.is_ascii_alphanumeric() || character == '_' {
			fragment.push(character.to_ascii_lowercase());
		} else {
			fragment.push('_');
		}
	}

	if fragment.is_empty() {
		fragment.push_str("table");
	} else if fragment
		.as_bytes()
		.first()
		.is_some_and(|character| character.is_ascii_digit())
	{
		fragment.insert_str(0, "table_");
	}
	fragment
}

fn safe_constraint_table_fragment(value: &str) -> String {
	let fragment = safe_constraint_name_fragment(value);
	if fragment == value {
		return fragment;
	}
	format!("{fragment}_{:08x}", stable_constraint_name_hash(value))
}

pub(crate) fn stable_constraint_name_hash(value: &str) -> u32 {
	let mut hash = 0x811c9dc5_u32;
	for byte in value.bytes() {
		hash ^= u32::from(byte);
		hash = hash.wrapping_mul(0x01000193);
	}
	hash
}

fn bounded_constraint_identifier(value: &str) -> String {
	const MAX_CONSTRAINT_IDENTIFIER_BYTES: usize = 63;
	if value.len() <= MAX_CONSTRAINT_IDENTIFIER_BYTES {
		return value.to_owned();
	}

	let suffix = format!("_{:08x}", stable_constraint_name_hash(value));
	let prefix_len = MAX_CONSTRAINT_IDENTIFIER_BYTES - suffix.len();
	let mut end = prefix_len;
	while !value.is_char_boundary(end) {
		end -= 1;
	}
	format!("{}{}", &value[..end], suffix)
}

#[cfg(test)]
mod tests {
	use super::{
		enum_domain_constraint_name, foreign_key_constraint_name, generated_unique_constraint_names,
	};

	#[test]
	fn generated_unique_names_are_stable_bounded_and_collision_safe() {
		let fields = vec!["email_addr".to_owned(), "display_name".to_owned()];
		let first = generated_unique_constraint_names("accounts", &fields, &[]);
		let reserved = vec![first[0].0.to_ascii_uppercase()];
		let collided = generated_unique_constraint_names("accounts", &fields, &reserved);

		assert_eq!(
			first,
			generated_unique_constraint_names("accounts", &fields, &[])
		);
		assert!(first.iter().all(|(name, _)| name.len() <= 63));
		assert_ne!(
			first[0].0.to_ascii_lowercase(),
			collided[0].0.to_ascii_lowercase()
		);
		assert_eq!(collided[0].1, "display_name");
	}

	#[test]
	fn generated_fk_and_domain_names_use_physical_columns() {
		assert_eq!(
			foreign_key_constraint_name("posts", "author_key"),
			"fk_posts_author_key"
		);
		assert_eq!(
			enum_domain_constraint_name("jobs", "status"),
			"jobs_status_model_enum_check"
		);
	}
}
