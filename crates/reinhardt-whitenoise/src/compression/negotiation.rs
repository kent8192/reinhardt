//! Accept-Encoding header parsing and content negotiation

/// Parses Accept-Encoding header
///
/// # Arguments
///
/// * `header` - Accept-Encoding header value
///
/// # Returns
///
/// Tuple of (supports_brotli, supports_gzip)
pub fn parse_accept_encoding(header: &str) -> (bool, bool) {
	let header = header.to_lowercase();
	let supports_brotli = header.contains("br");
	let supports_gzip = header.contains("gzip");
	(supports_brotli, supports_gzip)
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[case("br, gzip", true, true)]
	#[case("gzip", false, true)]
	#[case("br", true, false)]
	#[case("identity", false, false)]
	#[case("gzip, deflate, br", true, true)]
	fn test_parse_accept_encoding(
		#[case] header: &str,
		#[case] expect_br: bool,
		#[case] expect_gzip: bool,
	) {
		let (supports_br, supports_gzip) = parse_accept_encoding(header);
		assert_eq!(supports_br, expect_br);
		assert_eq!(supports_gzip, expect_gzip);
	}
}
