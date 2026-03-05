//! Fuzzy search filter
//!
//! Provides fuzzy string matching with configurable similarity algorithms.

use std::marker::PhantomData;

/// Fuzzy search algorithm
///
/// Determines how similarity is calculated between strings.
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::FuzzyAlgorithm;
///
/// let levenshtein = FuzzyAlgorithm::Levenshtein;
/// let jaro_winkler = FuzzyAlgorithm::JaroWinkler;
/// let trigram = FuzzyAlgorithm::Trigram;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FuzzyAlgorithm {
	/// Levenshtein distance
	///
	/// Measures the minimum number of single-character edits
	/// (insertions, deletions, or substitutions) required to
	/// change one string into another.
	#[default]
	Levenshtein,

	/// Damerau-Levenshtein distance
	///
	/// Extension of Levenshtein that also considers transpositions
	/// (swapping of two adjacent characters).
	DamerauLevenshtein,

	/// Jaro-Winkler similarity
	///
	/// Measures similarity based on matching characters and transpositions,
	/// with a prefix scale giving more favorable ratings to strings with
	/// common prefixes.
	JaroWinkler,

	/// Trigram similarity
	///
	/// Measures similarity based on the number of common three-character
	/// sequences (trigrams) between strings.
	Trigram,

	/// Soundex
	///
	/// Phonetic algorithm for indexing names by sound.
	/// Useful for matching names despite spelling variations.
	Soundex,

	/// Metaphone
	///
	/// More sophisticated phonetic algorithm than Soundex.
	/// Better handles consonant sounds and pronunciation.
	Metaphone,
}

/// Fuzzy search filter
///
/// Provides fuzzy string matching for inexact queries.
///
/// # Type Parameters
///
/// * `M` - The model type being searched
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::{FuzzySearchFilter, FuzzyAlgorithm};
///
/// #[derive(Clone)]
/// struct User {
///     id: i64,
///     name: String,
///     email: String,
/// }
///
/// let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new()
///     .query("Jon Doe")
///     .field("name")
///     .threshold(0.8)
///     .algorithm(FuzzyAlgorithm::JaroWinkler);
/// ```
#[derive(Debug, Clone)]
pub struct FuzzySearchFilter<M> {
	/// The search query
	pub query: String,
	/// Field to search in
	pub field: String,
	/// Similarity threshold (0.0 - 1.0)
	pub threshold: f64,
	/// Fuzzy matching algorithm
	pub algorithm: FuzzyAlgorithm,
	/// Maximum edit distance for Levenshtein-based algorithms
	pub max_distance: Option<usize>,
	/// Case sensitivity
	pub case_sensitive: bool,
	/// Prefix length for Jaro-Winkler
	pub prefix_length: usize,
	_phantom: PhantomData<M>,
}

impl<M> FuzzySearchFilter<M> {
	/// Create a new fuzzy search filter
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::FuzzySearchFilter;
	///
	/// #[derive(Clone)]
	/// struct Article {
	///     id: i64,
	/// }
	///
	/// let filter: FuzzySearchFilter<Article> = FuzzySearchFilter::new();
	/// ```
	pub fn new() -> Self {
		Self {
			query: String::new(),
			field: String::new(),
			threshold: 0.8,
			algorithm: FuzzyAlgorithm::default(),
			max_distance: None,
			case_sensitive: false,
			prefix_length: 4,
			_phantom: PhantomData,
		}
	}

	/// Set the search query
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::FuzzySearchFilter;
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new()
	///     .query("Jon Doe");
	/// assert_eq!(filter.query, "Jon Doe");
	/// ```
	pub fn query(mut self, query: impl Into<String>) -> Self {
		self.query = query.into();
		self
	}

	/// Set the field to search in
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::FuzzySearchFilter;
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new()
	///     .field("name");
	/// assert_eq!(filter.field, "name");
	/// ```
	pub fn field(mut self, field: impl Into<String>) -> Self {
		self.field = field.into();
		self
	}

	/// Set the similarity threshold
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::FuzzySearchFilter;
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new()
	///     .threshold(0.9);
	/// assert_eq!(filter.threshold, 0.9);
	/// ```
	pub fn threshold(mut self, threshold: f64) -> Self {
		self.threshold = threshold.clamp(0.0, 1.0);
		self
	}

	/// Set the fuzzy matching algorithm
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::{FuzzySearchFilter, FuzzyAlgorithm};
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new()
	///     .algorithm(FuzzyAlgorithm::JaroWinkler);
	/// assert_eq!(filter.algorithm, FuzzyAlgorithm::JaroWinkler);
	/// ```
	pub fn algorithm(mut self, algorithm: FuzzyAlgorithm) -> Self {
		self.algorithm = algorithm;
		self
	}

	/// Set the maximum edit distance
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::FuzzySearchFilter;
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new()
	///     .max_distance(2);
	/// assert_eq!(filter.max_distance, Some(2));
	/// ```
	pub fn max_distance(mut self, distance: usize) -> Self {
		self.max_distance = Some(distance);
		self
	}

	/// Set case sensitivity
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::FuzzySearchFilter;
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new()
	///     .case_sensitive(true);
	/// assert!(filter.is_case_sensitive());
	/// ```
	pub fn case_sensitive(mut self, sensitive: bool) -> Self {
		self.case_sensitive = sensitive;
		self
	}

	/// Set the prefix length for Jaro-Winkler
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::FuzzySearchFilter;
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new()
	///     .prefix_length(6);
	/// assert_eq!(filter.prefix_length, 6);
	/// ```
	pub fn prefix_length(mut self, length: usize) -> Self {
		self.prefix_length = length;
		self
	}

	/// Check if case sensitive
	pub fn is_case_sensitive(&self) -> bool {
		self.case_sensitive
	}

	/// Calculate similarity between two strings
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::{FuzzySearchFilter, FuzzyAlgorithm};
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new()
	///     .algorithm(FuzzyAlgorithm::Levenshtein);
	///
	/// let similarity = filter.calculate_similarity("kitten", "sitting");
	/// assert!(similarity >= 0.0 && similarity <= 1.0);
	/// ```
	pub fn calculate_similarity(&self, a: &str, b: &str) -> f64 {
		let a = if self.case_sensitive {
			a.to_string()
		} else {
			a.to_lowercase()
		};
		let b = if self.case_sensitive {
			b.to_string()
		} else {
			b.to_lowercase()
		};

		match self.algorithm {
			FuzzyAlgorithm::Levenshtein => self.levenshtein_similarity(&a, &b),
			FuzzyAlgorithm::DamerauLevenshtein => self.damerau_levenshtein_similarity(&a, &b),
			FuzzyAlgorithm::JaroWinkler => self.jaro_winkler_similarity(&a, &b),
			FuzzyAlgorithm::Trigram => self.trigram_similarity(&a, &b),
			FuzzyAlgorithm::Soundex => self.soundex_similarity(&a, &b),
			FuzzyAlgorithm::Metaphone => self.metaphone_similarity(&a, &b),
		}
	}

	/// Check if two strings match within the threshold
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::FuzzySearchFilter;
	///
	/// #[derive(Clone)]
	/// struct User {
	///     id: i64,
	/// }
	///
	/// let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new()
	///     .threshold(0.8);
	///
	/// assert!(filter.matches("John Doe", "Jon Doe"));
	/// ```
	pub fn matches(&self, a: &str, b: &str) -> bool {
		let similarity = self.calculate_similarity(a, b);
		similarity >= self.threshold
	}

	fn levenshtein_similarity(&self, a: &str, b: &str) -> f64 {
		let distance = self.levenshtein_distance(a, b);
		let max_len = a.len().max(b.len());
		if max_len == 0 {
			return 1.0;
		}
		1.0 - (distance as f64 / max_len as f64)
	}

	fn levenshtein_distance(&self, a: &str, b: &str) -> usize {
		let a_chars: Vec<char> = a.chars().collect();
		let b_chars: Vec<char> = b.chars().collect();
		let a_len = a_chars.len();
		let b_len = b_chars.len();

		if a_len == 0 {
			return b_len;
		}
		if b_len == 0 {
			return a_len;
		}

		let mut prev_row: Vec<usize> = (0..=b_len).collect();
		let mut curr_row = vec![0; b_len + 1];

		for i in 1..=a_len {
			curr_row[0] = i;
			for j in 1..=b_len {
				let cost = if a_chars[i - 1] == b_chars[j - 1] {
					0
				} else {
					1
				};
				curr_row[j] = (curr_row[j - 1] + 1)
					.min(prev_row[j] + 1)
					.min(prev_row[j - 1] + cost);
			}
			std::mem::swap(&mut prev_row, &mut curr_row);
		}

		prev_row[b_len]
	}

	/// Compute Damerau-Levenshtein distance between two strings.
	///
	/// This extends Levenshtein distance by also considering transpositions
	/// (swapping of two adjacent characters) as a single operation.
	fn damerau_levenshtein_distance(&self, a: &str, b: &str) -> usize {
		let a_chars: Vec<char> = a.chars().collect();
		let b_chars: Vec<char> = b.chars().collect();
		let a_len = a_chars.len();
		let b_len = b_chars.len();

		if a_len == 0 {
			return b_len;
		}
		if b_len == 0 {
			return a_len;
		}

		// Need 3 rows for transposition: prev_prev, prev, curr
		let mut prev_prev_row: Vec<usize> = vec![0; b_len + 1];
		let mut prev_row: Vec<usize> = (0..=b_len).collect();
		let mut curr_row = vec![0; b_len + 1];

		for i in 1..=a_len {
			curr_row[0] = i;
			for j in 1..=b_len {
				let cost = if a_chars[i - 1] == b_chars[j - 1] {
					0
				} else {
					1
				};

				curr_row[j] = (curr_row[j - 1] + 1)
					.min(prev_row[j] + 1)
					.min(prev_row[j - 1] + cost);

				// Transposition: check if swapping adjacent characters helps
				if i > 1
					&& j > 1 && a_chars[i - 1] == b_chars[j - 2]
					&& a_chars[i - 2] == b_chars[j - 1]
				{
					curr_row[j] = curr_row[j].min(prev_prev_row[j - 2] + cost);
				}
			}
			std::mem::swap(&mut prev_prev_row, &mut prev_row);
			std::mem::swap(&mut prev_row, &mut curr_row);
		}

		prev_row[b_len]
	}

	fn damerau_levenshtein_similarity(&self, a: &str, b: &str) -> f64 {
		let distance = self.damerau_levenshtein_distance(a, b);
		let max_len = a.len().max(b.len());
		if max_len == 0 {
			return 1.0;
		}
		1.0 - (distance as f64 / max_len as f64)
	}

	fn jaro_winkler_similarity(&self, a: &str, b: &str) -> f64 {
		let jaro = self.jaro_similarity(a, b);
		let prefix_len = self.common_prefix_length(a, b).min(self.prefix_length);
		jaro + (prefix_len as f64 * 0.1 * (1.0 - jaro))
	}

	fn jaro_similarity(&self, a: &str, b: &str) -> f64 {
		let a_chars: Vec<char> = a.chars().collect();
		let b_chars: Vec<char> = b.chars().collect();
		let a_len = a_chars.len();
		let b_len = b_chars.len();

		if a_len == 0 && b_len == 0 {
			return 1.0;
		}
		if a_len == 0 || b_len == 0 {
			return 0.0;
		}

		let match_window = (a_len.max(b_len) / 2).saturating_sub(1);
		let mut a_matches = vec![false; a_len];
		let mut b_matches = vec![false; b_len];
		let mut matches = 0.0;
		let mut transpositions = 0.0;

		for i in 0..a_len {
			let start = i.saturating_sub(match_window);
			let end = (i + match_window + 1).min(b_len);
			for j in start..end {
				if !b_matches[j] && a_chars[i] == b_chars[j] {
					a_matches[i] = true;
					b_matches[j] = true;
					matches += 1.0;
					break;
				}
			}
		}

		if matches == 0.0 {
			return 0.0;
		}

		let mut k = 0;
		for i in 0..a_len {
			if a_matches[i] {
				while !b_matches[k] {
					k += 1;
				}
				if a_chars[i] != b_chars[k] {
					transpositions += 1.0;
				}
				k += 1;
			}
		}

		(matches / a_len as f64
			+ matches / b_len as f64
			+ (matches - transpositions / 2.0) / matches)
			/ 3.0
	}

	fn trigram_similarity(&self, a: &str, b: &str) -> f64 {
		let a_trigrams = self.trigrams(a);
		let b_trigrams = self.trigrams(b);

		if a_trigrams.is_empty() && b_trigrams.is_empty() {
			return 1.0;
		}
		if a_trigrams.is_empty() || b_trigrams.is_empty() {
			return 0.0;
		}

		let common = a_trigrams.iter().filter(|t| b_trigrams.contains(t)).count();
		let total = a_trigrams.len() + b_trigrams.len();

		(2.0 * common as f64) / total as f64
	}

	fn trigrams(&self, s: &str) -> Vec<String> {
		let chars: Vec<char> = s.chars().collect();
		if chars.len() < 3 {
			return vec![];
		}
		(0..=chars.len() - 3)
			.map(|i| chars[i..i + 3].iter().collect())
			.collect()
	}

	fn soundex_similarity(&self, a: &str, b: &str) -> f64 {
		if self.soundex_code(a) == self.soundex_code(b) {
			1.0
		} else {
			0.0
		}
	}

	fn soundex_code(&self, s: &str) -> String {
		let chars: Vec<char> = s.chars().filter(|c| c.is_alphabetic()).collect();
		if chars.is_empty() {
			return String::from("0000");
		}

		let first = chars[0].to_uppercase().next().unwrap();
		let mut code = String::from(first);
		let mut prev_code = self.soundex_digit(first);

		for &c in &chars[1..] {
			let digit = self.soundex_digit(c);
			if digit != '0' && digit != prev_code {
				code.push(digit);
				if code.len() == 4 {
					break;
				}
			}
			if digit != '0' {
				prev_code = digit;
			}
		}

		while code.len() < 4 {
			code.push('0');
		}

		code
	}

	fn soundex_digit(&self, c: char) -> char {
		match c.to_uppercase().next().unwrap() {
			'B' | 'F' | 'P' | 'V' => '1',
			'C' | 'G' | 'J' | 'K' | 'Q' | 'S' | 'X' | 'Z' => '2',
			'D' | 'T' => '3',
			'L' => '4',
			'M' | 'N' => '5',
			'R' => '6',
			_ => '0',
		}
	}

	/// Check if a character is a vowel (A, E, I, O, U).
	fn is_vowel(&self, c: char) -> bool {
		matches!(c, 'A' | 'E' | 'I' | 'O' | 'U')
	}

	/// Compute the Original Metaphone code for a string.
	///
	/// Metaphone is a phonetic algorithm that encodes words by their
	/// English pronunciation, providing better phonetic matching than Soundex.
	fn metaphone_code(&self, s: &str) -> String {
		let chars: Vec<char> = s
			.chars()
			.filter(|c| c.is_alphabetic())
			.flat_map(|c| c.to_uppercase())
			.collect();

		if chars.is_empty() {
			return String::new();
		}

		let len = chars.len();
		let mut result = String::new();
		let mut i = 0;

		// Helper to get character at position safely
		let get_char = |pos: usize| -> Option<char> { chars.get(pos).copied() };

		// Helper to check if position has a specific character
		let is_char_at = |pos: usize, c: char| -> bool { get_char(pos) == Some(c) };

		// Handle initial letter combinations
		if len >= 2 {
			match (chars[0], chars[1]) {
				('K', 'N') | ('G', 'N') | ('P', 'N') | ('A', 'E') | ('W', 'R') => {
					i = 1;
				}
				('W', 'H') => {
					result.push('W');
					i = 2;
				}
				('X', _) => {
					result.push('S');
					i = 1;
				}
				_ => {}
			}
		}

		while i < len && result.len() < 6 {
			let c = chars[i];
			let next = get_char(i + 1);
			let next2 = get_char(i + 2);
			let prev = if i > 0 { get_char(i - 1) } else { None };

			match c {
				'A' | 'E' | 'I' | 'O' | 'U' => {
					// Vowels only coded at the beginning
					if i == 0 {
						result.push(c);
					}
				}
				'B' => {
					// B is silent if at end after M
					if !(i == len - 1 && prev == Some('M')) {
						result.push('B');
					}
				}
				'C' => {
					if next == Some('H') {
						// CH -> X (like "sh")
						result.push('X');
						i += 1;
					} else if next == Some('I') || next == Some('E') || next == Some('Y') {
						// C before I, E, Y -> S
						result.push('S');
					} else {
						// C -> K
						result.push('K');
					}
				}
				'D' => {
					if next == Some('G')
						&& (next2 == Some('E') || next2 == Some('Y') || next2 == Some('I'))
					{
						// DGE, DGY, DGI -> J
						result.push('J');
						i += 2;
					} else {
						result.push('T');
					}
				}
				'F' => {
					result.push('F');
				}
				'G' => {
					if next == Some('H') {
						// GH is often silent
						if i + 2 < len && !self.is_vowel(chars[i + 2]) {
							// GH before consonant is silent
							i += 1;
						} else if i == 0 {
							// GH at start: encode as K if followed by vowel
							if next2.is_some() && self.is_vowel(next2.unwrap()) {
								result.push('K');
								i += 1;
							}
						} else {
							i += 1;
						}
					} else if next == Some('N') {
						// GN at end or before consonant is silent
						if i + 2 >= len || !self.is_vowel(chars[i + 2]) {
							// Skip G, N will be processed
						} else {
							result.push('K');
						}
					} else if next == Some('I') || next == Some('E') || next == Some('Y') {
						// G before I, E, Y -> J
						result.push('J');
					} else {
						result.push('K');
					}
				}
				'H' => {
					// H is coded if followed by vowel and not after vowel or certain consonants
					let after_vowel_or_special =
						prev.is_some() && (self.is_vowel(prev.unwrap()) || prev == Some('C'));
					if !after_vowel_or_special && next.is_some() && self.is_vowel(next.unwrap()) {
						result.push('H');
					}
				}
				'J' => {
					result.push('J');
				}
				'K' => {
					// K after C is silent
					if prev != Some('C') {
						result.push('K');
					}
				}
				'L' => {
					result.push('L');
				}
				'M' => {
					result.push('M');
				}
				'N' => {
					result.push('N');
				}
				'P' => {
					if next == Some('H') {
						result.push('F');
						i += 1;
					} else {
						result.push('P');
					}
				}
				'Q' => {
					result.push('K');
				}
				'R' => {
					result.push('R');
				}
				'S' => {
					if next == Some('H') {
						result.push('X');
						i += 1;
					} else if next == Some('I') && (next2 == Some('O') || next2 == Some('A')) {
						// SIO, SIA -> X
						result.push('X');
					} else {
						result.push('S');
					}
				}
				'T' => {
					if next == Some('H') {
						// TH -> 0 (theta sound)
						result.push('0');
						i += 1;
					} else if next == Some('I') && (next2 == Some('O') || next2 == Some('A')) {
						// TIO, TIA -> X
						result.push('X');
					} else if next == Some('C') && is_char_at(i + 2, 'H') {
						// TCH -> skip T
					} else {
						result.push('T');
					}
				}
				'V' => {
					result.push('F');
				}
				'W' => {
					// W only coded if followed by vowel
					if next.is_some() && self.is_vowel(next.unwrap()) {
						result.push('W');
					}
				}
				'X' => {
					result.push('K');
					result.push('S');
				}
				'Y' => {
					// Y only coded if followed by vowel
					if next.is_some() && self.is_vowel(next.unwrap()) {
						result.push('Y');
					}
				}
				'Z' => {
					result.push('S');
				}
				_ => {}
			}
			i += 1;
		}

		result
	}

	/// Calculate Metaphone similarity using Levenshtein distance on codes.
	///
	/// Unlike Soundex which returns binary match/no-match, Metaphone similarity
	/// provides graduated matching based on how similar the phonetic codes are.
	fn metaphone_similarity(&self, a: &str, b: &str) -> f64 {
		let code_a = self.metaphone_code(a);
		let code_b = self.metaphone_code(b);

		if code_a.is_empty() && code_b.is_empty() {
			return 1.0;
		}
		if code_a.is_empty() || code_b.is_empty() {
			return 0.0;
		}

		let distance = self.levenshtein_distance(&code_a, &code_b);
		let max_len = code_a.len().max(code_b.len());
		1.0 - (distance as f64 / max_len as f64)
	}

	fn common_prefix_length(&self, a: &str, b: &str) -> usize {
		let a_chars: Vec<char> = a.chars().collect();
		let b_chars: Vec<char> = b.chars().collect();
		a_chars
			.iter()
			.zip(b_chars.iter())
			.take_while(|(a, b)| a == b)
			.count()
	}
}

impl<M> Default for FuzzySearchFilter<M> {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[derive(Clone)]
	#[allow(dead_code)]
	struct User {
		id: i64,
		name: String,
	}

	#[test]
	fn test_fuzzy_algorithm_default() {
		assert_eq!(FuzzyAlgorithm::default(), FuzzyAlgorithm::Levenshtein);
	}

	#[test]
	fn test_filter_creation() {
		let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new();
		assert_eq!(filter.query, "");
		assert_eq!(filter.field, "");
		assert_eq!(filter.threshold, 0.8);
		assert_eq!(filter.algorithm, FuzzyAlgorithm::Levenshtein);
		assert_eq!(filter.max_distance, None);
		assert!(!filter.is_case_sensitive());
	}

	#[test]
	fn test_filter_builder() {
		let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new()
			.query("Jon Doe")
			.field("name")
			.threshold(0.9)
			.algorithm(FuzzyAlgorithm::JaroWinkler)
			.max_distance(2)
			.case_sensitive(true)
			.prefix_length(6);

		assert_eq!(filter.query, "Jon Doe");
		assert_eq!(filter.field, "name");
		assert_eq!(filter.threshold, 0.9);
		assert_eq!(filter.algorithm, FuzzyAlgorithm::JaroWinkler);
		assert_eq!(filter.max_distance, Some(2));
		assert!(filter.is_case_sensitive());
		assert_eq!(filter.prefix_length, 6);
	}

	#[test]
	fn test_threshold_clamping() {
		let filter1: FuzzySearchFilter<User> = FuzzySearchFilter::new().threshold(1.5);
		assert_eq!(filter1.threshold, 1.0);

		let filter2: FuzzySearchFilter<User> = FuzzySearchFilter::new().threshold(-0.5);
		assert_eq!(filter2.threshold, 0.0);
	}

	#[test]
	fn test_levenshtein_distance() {
		let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new();
		assert_eq!(filter.levenshtein_distance("kitten", "sitting"), 3);
		assert_eq!(filter.levenshtein_distance("saturday", "sunday"), 3);
		assert_eq!(filter.levenshtein_distance("", "abc"), 3);
		assert_eq!(filter.levenshtein_distance("abc", ""), 3);
	}

	#[test]
	fn test_levenshtein_similarity() {
		let filter: FuzzySearchFilter<User> =
			FuzzySearchFilter::new().algorithm(FuzzyAlgorithm::Levenshtein);

		assert_eq!(filter.calculate_similarity("abc", "abc"), 1.0);
		assert!(filter.calculate_similarity("kitten", "sitting") > 0.5);
	}

	#[test]
	fn test_jaro_winkler_similarity() {
		let filter: FuzzySearchFilter<User> =
			FuzzySearchFilter::new().algorithm(FuzzyAlgorithm::JaroWinkler);

		assert_eq!(filter.calculate_similarity("abc", "abc"), 1.0);
		assert!(filter.calculate_similarity("martha", "marhta") > 0.9);
		assert!(filter.calculate_similarity("dixon", "dicksonx") > 0.7);
	}

	#[test]
	fn test_trigram_similarity() {
		let filter: FuzzySearchFilter<User> =
			FuzzySearchFilter::new().algorithm(FuzzyAlgorithm::Trigram);

		assert_eq!(filter.calculate_similarity("abc", "abc"), 1.0);
		// "hello" vs "hellow" shares 2 trigrams: "hel", "ell", "llo" vs "hel", "ell", "llo", "low"
		// common = 3, total = 3 + 4 = 7, similarity = 6/7 ≈ 0.857
		assert!(filter.calculate_similarity("hello", "hellow") > 0.5);
	}

	#[test]
	fn test_soundex_code() {
		let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new();
		assert_eq!(filter.soundex_code("Smith"), "S530");
		assert_eq!(filter.soundex_code("Smythe"), "S530");
		assert_eq!(filter.soundex_code("Johnson"), "J525");
		assert_eq!(filter.soundex_code("Jonson"), "J525");
	}

	#[test]
	fn test_soundex_similarity() {
		let filter: FuzzySearchFilter<User> =
			FuzzySearchFilter::new().algorithm(FuzzyAlgorithm::Soundex);

		assert_eq!(filter.calculate_similarity("Smith", "Smythe"), 1.0);
		assert_eq!(filter.calculate_similarity("Johnson", "Jonson"), 1.0);
		assert_eq!(filter.calculate_similarity("Smith", "Johnson"), 0.0);
	}

	#[test]
	fn test_matches() {
		let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new()
			.threshold(0.8)
			.algorithm(FuzzyAlgorithm::Levenshtein);

		assert!(filter.matches("hello", "hello"));
		assert!(filter.matches("hello", "helo"));
		assert!(!filter.matches("hello", "world"));
	}

	#[test]
	fn test_case_sensitivity() {
		let sensitive: FuzzySearchFilter<User> = FuzzySearchFilter::new()
			.case_sensitive(true)
			.algorithm(FuzzyAlgorithm::Levenshtein);

		let insensitive: FuzzySearchFilter<User> = FuzzySearchFilter::new()
			.case_sensitive(false)
			.algorithm(FuzzyAlgorithm::Levenshtein);

		assert!(sensitive.calculate_similarity("Hello", "hello") < 1.0);
		assert_eq!(insensitive.calculate_similarity("Hello", "hello"), 1.0);
	}

	#[test]
	fn test_get_trigrams() {
		let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new();
		let trigrams = filter.trigrams("hello");
		assert_eq!(trigrams.len(), 3);
		assert!(trigrams.contains(&"hel".to_string()));
		assert!(trigrams.contains(&"ell".to_string()));
		assert!(trigrams.contains(&"llo".to_string()));
	}

	#[test]
	fn test_common_prefix_length() {
		let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new();
		assert_eq!(filter.common_prefix_length("hello", "help"), 3);
		assert_eq!(filter.common_prefix_length("abc", "def"), 0);
		assert_eq!(filter.common_prefix_length("test", "test"), 4);
	}

	#[test]
	fn test_empty_strings() {
		let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new();
		assert_eq!(filter.calculate_similarity("", ""), 1.0);
		assert_eq!(filter.calculate_similarity("", "abc"), 0.0);
		assert_eq!(filter.calculate_similarity("abc", ""), 0.0);
	}

	#[test]
	fn test_damerau_levenshtein_distance() {
		let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new();

		// Basic cases
		assert_eq!(filter.damerau_levenshtein_distance("", ""), 0);
		assert_eq!(filter.damerau_levenshtein_distance("abc", ""), 3);
		assert_eq!(filter.damerau_levenshtein_distance("", "abc"), 3);
		assert_eq!(filter.damerau_levenshtein_distance("abc", "abc"), 0);

		// Single character operations
		assert_eq!(filter.damerau_levenshtein_distance("abc", "ab"), 1); // deletion
		assert_eq!(filter.damerau_levenshtein_distance("ab", "abc"), 1); // insertion
		assert_eq!(filter.damerau_levenshtein_distance("abc", "adc"), 1); // substitution

		// Transposition (the key difference from Levenshtein)
		// Damerau-Levenshtein treats "ab" -> "ba" as 1 transposition
		assert_eq!(filter.damerau_levenshtein_distance("ab", "ba"), 1);
		assert_eq!(filter.damerau_levenshtein_distance("abc", "bac"), 1);

		// Compare with Levenshtein: transposition should be cheaper
		// Levenshtein: "ab" -> "ba" requires 2 operations (substitute twice)
		assert_eq!(filter.levenshtein_distance("ab", "ba"), 2);
		assert!(
			filter.damerau_levenshtein_distance("ab", "ba")
				< filter.levenshtein_distance("ab", "ba")
		);

		// Common typos that benefit from transposition
		assert_eq!(filter.damerau_levenshtein_distance("teh", "the"), 1);
		assert_eq!(filter.damerau_levenshtein_distance("recieve", "receive"), 1);
	}

	#[test]
	fn test_damerau_levenshtein_similarity() {
		let filter: FuzzySearchFilter<User> =
			FuzzySearchFilter::new().algorithm(FuzzyAlgorithm::DamerauLevenshtein);

		// Identical strings
		assert_eq!(filter.calculate_similarity("test", "test"), 1.0);
		assert_eq!(filter.calculate_similarity("", ""), 1.0);

		// Transposed strings should have high similarity
		let sim_transposed = filter.calculate_similarity("ab", "ba");
		assert!(
			sim_transposed >= 0.5,
			"Transposed should have >= 0.5 similarity"
		);

		// Common typos
		let sim_typo = filter.calculate_similarity("the", "teh");
		assert!(sim_typo >= 0.6, "Common typo should have high similarity");

		// Compare with Levenshtein for transposition
		let levenshtein_filter: FuzzySearchFilter<User> =
			FuzzySearchFilter::new().algorithm(FuzzyAlgorithm::Levenshtein);
		let dl_sim = filter.calculate_similarity("ab", "ba");
		let lev_sim = levenshtein_filter.calculate_similarity("ab", "ba");
		assert!(
			dl_sim >= lev_sim,
			"DL should be >= Levenshtein for transpositions"
		);
	}

	#[test]
	fn test_metaphone_code() {
		let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new();

		// Empty and non-alphabetic
		assert_eq!(filter.metaphone_code(""), "");
		assert_eq!(filter.metaphone_code("123"), "");

		// Initial letter combinations
		assert_eq!(filter.metaphone_code("knife"), "NF"); // KN -> N
		assert_eq!(filter.metaphone_code("gnat"), "NT"); // GN -> N
		assert_eq!(filter.metaphone_code("phone"), "FN"); // PH -> F
		assert_eq!(filter.metaphone_code("write"), "RT"); // WR -> R
		assert_eq!(filter.metaphone_code("what"), "WT"); // WH -> W

		// Common consonant patterns
		assert_eq!(filter.metaphone_code("ship"), "XP"); // SH -> X
		assert_eq!(filter.metaphone_code("church"), "XRX"); // CH -> X
		assert_eq!(filter.metaphone_code("thing"), "0NK"); // TH -> 0 (theta)

		// C variations
		assert_eq!(filter.metaphone_code("cat"), "KT"); // C -> K
		assert_eq!(filter.metaphone_code("city"), "ST"); // C before I -> S
		assert_eq!(filter.metaphone_code("cent"), "SNT"); // C before E -> S

		// G variations
		assert_eq!(filter.metaphone_code("go"), "K"); // G -> K
		assert_eq!(filter.metaphone_code("gem"), "JM"); // G before E -> J
		assert_eq!(filter.metaphone_code("giant"), "JNT"); // G before I -> J

		// Silent letters
		assert_eq!(filter.metaphone_code("lamb"), "LM"); // B silent after M
		assert_eq!(filter.metaphone_code("back"), "BK"); // CK -> K (K after C is silent)

		// X handling
		assert_eq!(filter.metaphone_code("box"), "BKS"); // X -> KS

		// Names (common use case)
		let smith = filter.metaphone_code("Smith");
		let smythe = filter.metaphone_code("Smythe");
		assert_eq!(smith, smythe); // Both should produce same code

		// V -> F
		assert_eq!(filter.metaphone_code("voice"), "FS"); // V -> F
	}

	#[test]
	fn test_metaphone_similarity() {
		let filter: FuzzySearchFilter<User> =
			FuzzySearchFilter::new().algorithm(FuzzyAlgorithm::Metaphone);

		// Identical strings
		assert_eq!(filter.calculate_similarity("test", "test"), 1.0);

		// Empty strings
		assert_eq!(filter.calculate_similarity("", ""), 1.0);
		assert_eq!(filter.calculate_similarity("", "test"), 0.0);
		assert_eq!(filter.calculate_similarity("test", ""), 0.0);

		// Phonetically identical names
		assert_eq!(filter.calculate_similarity("Smith", "Smythe"), 1.0);

		// Similar sounding words should have high similarity
		let sim_phone_fone = filter.calculate_similarity("phone", "fone");
		assert!(
			sim_phone_fone > 0.5,
			"Phonetically similar words should match well"
		);

		// Graduated similarity (unlike binary Soundex)
		let sim_similar = filter.calculate_similarity("smith", "smit");
		let sim_different = filter.calculate_similarity("smith", "jones");
		assert!(
			sim_similar > sim_different,
			"Similar words should have higher similarity than different words"
		);
	}

	#[test]
	fn test_metaphone_vs_soundex() {
		let filter: FuzzySearchFilter<User> = FuzzySearchFilter::new();

		// Metaphone produces graduated similarity, Soundex is binary
		let soundex_filter: FuzzySearchFilter<User> =
			FuzzySearchFilter::new().algorithm(FuzzyAlgorithm::Soundex);
		let metaphone_filter: FuzzySearchFilter<User> =
			FuzzySearchFilter::new().algorithm(FuzzyAlgorithm::Metaphone);

		// Soundex gives binary 0.0 or 1.0
		let soundex_sim = soundex_filter.calculate_similarity("Smith", "Smythe");
		assert!(
			soundex_sim == 0.0 || soundex_sim == 1.0,
			"Soundex should be binary"
		);

		// Metaphone can give graduated values
		// (though exact matches still give 1.0)
		let meta_exact = metaphone_filter.calculate_similarity("test", "test");
		assert_eq!(meta_exact, 1.0);

		// Metaphone handles more complex phonetics
		// Test that Metaphone codes are different for genuinely different sounds
		let code_smith = filter.metaphone_code("Smith");
		let code_jones = filter.metaphone_code("Jones");
		assert_ne!(
			code_smith, code_jones,
			"Different names should have different codes"
		);

		// Metaphone handles silent letters better
		let code_knife = filter.metaphone_code("knife");
		let code_nife = filter.metaphone_code("nife");
		assert_eq!(code_knife, code_nife, "Silent K should be handled");

		// Metaphone handles PH = F
		let code_phone = filter.metaphone_code("phone");
		assert!(code_phone.starts_with('F'), "PH should be encoded as F");

		// Compare TH handling
		let code_the = filter.metaphone_code("the");
		assert!(code_the.contains('0'), "TH should be encoded as 0 (theta)");
	}
}
