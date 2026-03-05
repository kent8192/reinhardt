//! Faker integration for generating fake data.
//!
//! This module provides integration with the `fake` crate for generating
//! realistic test data.

use std::str::FromStr;

use fake::Fake;
use fake::faker::address::en::{CityName, CountryName, StateName, StreetName, ZipCode};
use fake::faker::company::en::CompanyName;
use fake::faker::internet::en::{DomainSuffix, FreeEmail, Password, SafeEmail, Username};
use fake::faker::lorem::en::{Paragraph, Sentence, Word, Words};
use fake::faker::name::en::{FirstName, LastName, Name};
use fake::faker::phone_number::en::{CellNumber, PhoneNumber};
use uuid::Uuid;

use crate::error::{SeedingError, SeedingResult};

/// Supported faker types for data generation.
///
/// Each variant represents a type of fake data that can be generated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FakerType {
	// Name types
	/// Full name (e.g., "John Smith")
	Name,
	/// First name only (e.g., "John")
	FirstName,
	/// Last name only (e.g., "Smith")
	LastName,

	// Internet types
	/// Email address (e.g., "john.smith@example.com")
	Email,
	/// Safe email with example.com domain
	SafeEmail,
	/// Username (e.g., "john_smith_42")
	Username,
	/// Password with configurable length
	Password,
	/// Domain name (e.g., "example.com")
	DomainName,
	/// URL (e.g., "https://example.com/page")
	Url,

	// Lorem types
	/// Single word
	Word,
	/// Multiple words (default 3-5)
	Words,
	/// Single sentence
	Sentence,
	/// Single paragraph
	Paragraph,

	// Address types
	/// Street name (e.g., "Main Street")
	StreetName,
	/// City name (e.g., "New York")
	City,
	/// State name (e.g., "California")
	State,
	/// Zip/postal code
	ZipCode,
	/// Country name
	Country,

	// Phone types
	/// Phone number
	PhoneNumber,
	/// Cell/mobile phone number
	CellNumber,

	// Company types
	/// Company name
	CompanyName,

	// Number types
	/// Random integer
	Integer,
	/// Random float
	Float,
	/// Random boolean
	Boolean,

	// Date/Time types
	/// Date in ISO format
	Date,
	/// DateTime in ISO format
	DateTime,
	/// Time in ISO format
	Time,

	// Identifier types
	/// UUID v4
	Uuid,
}

impl FakerType {
	/// Generates a fake value as a string.
	///
	/// # Returns
	///
	/// Returns the generated fake value as a `String`.
	pub fn generate(&self) -> String {
		match self {
			// Name types
			Self::Name => Name().fake(),
			Self::FirstName => FirstName().fake(),
			Self::LastName => LastName().fake(),

			// Internet types
			Self::Email => FreeEmail().fake(),
			Self::SafeEmail => SafeEmail().fake(),
			Self::Username => Username().fake(),
			Self::Password => Password(8..16).fake(),
			Self::DomainName => format!("example.{}", DomainSuffix().fake::<String>()),
			Self::Url => format!("https://example.{}/page", DomainSuffix().fake::<String>()),

			// Lorem types
			Self::Word => Word().fake(),
			Self::Words => Words(3..5).fake::<Vec<String>>().join(" "),
			Self::Sentence => Sentence(5..10).fake(),
			Self::Paragraph => Paragraph(3..5).fake(),

			// Address types
			Self::StreetName => StreetName().fake(),
			Self::City => CityName().fake(),
			Self::State => StateName().fake(),
			Self::ZipCode => ZipCode().fake(),
			Self::Country => CountryName().fake(),

			// Phone types
			Self::PhoneNumber => PhoneNumber().fake(),
			Self::CellNumber => CellNumber().fake(),

			// Company types
			Self::CompanyName => CompanyName().fake(),

			// Number types
			Self::Integer => format!("{}", (0..1000i32).fake::<i32>()),
			Self::Float => format!("{:.2}", (0.0..1000.0f64).fake::<f64>()),
			Self::Boolean => format!("{}", fake::rand::random::<bool>()),

			// Date/Time types
			Self::Date => {
				let year: i32 = (2020..2025).fake();
				let month: u32 = (1..=12).fake();
				let day: u32 = (1..=28).fake();
				format!("{:04}-{:02}-{:02}", year, month, day)
			}
			Self::DateTime => {
				let year: i32 = (2020..2025).fake();
				let month: u32 = (1..=12).fake();
				let day: u32 = (1..=28).fake();
				let hour: u32 = (0..24).fake();
				let minute: u32 = (0..60).fake();
				let second: u32 = (0..60).fake();
				format!(
					"{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
					year, month, day, hour, minute, second
				)
			}
			Self::Time => {
				let hour: u32 = (0..24).fake();
				let minute: u32 = (0..60).fake();
				let second: u32 = (0..60).fake();
				format!("{:02}:{:02}:{:02}", hour, minute, second)
			}

			// Identifier types
			Self::Uuid => Uuid::new_v4().to_string(),
		}
	}

	/// Generates a fake value with type conversion.
	///
	/// # Type Parameters
	///
	/// * `T` - Target type that implements `FromStr`
	pub fn generate_as<T: FromStr>(&self) -> SeedingResult<T>
	where
		T::Err: std::fmt::Display,
	{
		self.generate()
			.parse::<T>()
			.map_err(|e| SeedingError::FakerError(format!("Failed to convert faker value: {}", e)))
	}

	/// Returns all available faker type names.
	pub fn all_names() -> &'static [&'static str] {
		&[
			"name",
			"first_name",
			"last_name",
			"email",
			"safe_email",
			"username",
			"password",
			"domain_name",
			"url",
			"word",
			"words",
			"sentence",
			"paragraph",
			"street_name",
			"city",
			"state",
			"zip_code",
			"country",
			"phone_number",
			"cell_number",
			"company_name",
			"integer",
			"float",
			"boolean",
			"date",
			"datetime",
			"time",
			"uuid",
		]
	}
}

impl FromStr for FakerType {
	type Err = SeedingError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s.to_lowercase().as_str() {
			"name" => Ok(Self::Name),
			"first_name" | "firstname" => Ok(Self::FirstName),
			"last_name" | "lastname" => Ok(Self::LastName),
			"email" => Ok(Self::Email),
			"safe_email" | "safeemail" => Ok(Self::SafeEmail),
			"username" => Ok(Self::Username),
			"password" => Ok(Self::Password),
			"domain_name" | "domainname" | "domain" => Ok(Self::DomainName),
			"url" => Ok(Self::Url),
			"word" => Ok(Self::Word),
			"words" => Ok(Self::Words),
			"sentence" => Ok(Self::Sentence),
			"paragraph" => Ok(Self::Paragraph),
			"street_name" | "streetname" | "street" => Ok(Self::StreetName),
			"city" => Ok(Self::City),
			"state" => Ok(Self::State),
			"zip_code" | "zipcode" | "zip" | "postal_code" | "postalcode" => Ok(Self::ZipCode),
			"country" => Ok(Self::Country),
			"phone_number" | "phonenumber" | "phone" => Ok(Self::PhoneNumber),
			"cell_number" | "cellnumber" | "cell" | "mobile" => Ok(Self::CellNumber),
			"company_name" | "companyname" | "company" => Ok(Self::CompanyName),
			"integer" | "int" | "number" => Ok(Self::Integer),
			"float" | "decimal" | "double" => Ok(Self::Float),
			"boolean" | "bool" => Ok(Self::Boolean),
			"date" => Ok(Self::Date),
			"datetime" | "date_time" => Ok(Self::DateTime),
			"time" => Ok(Self::Time),
			"uuid" => Ok(Self::Uuid),
			other => Err(SeedingError::FakerError(format!(
				"Unknown faker type: '{}'. Available types: {:?}",
				other,
				Self::all_names()
			))),
		}
	}
}

impl std::fmt::Display for FakerType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let name = match self {
			Self::Name => "name",
			Self::FirstName => "first_name",
			Self::LastName => "last_name",
			Self::Email => "email",
			Self::SafeEmail => "safe_email",
			Self::Username => "username",
			Self::Password => "password",
			Self::DomainName => "domain_name",
			Self::Url => "url",
			Self::Word => "word",
			Self::Words => "words",
			Self::Sentence => "sentence",
			Self::Paragraph => "paragraph",
			Self::StreetName => "street_name",
			Self::City => "city",
			Self::State => "state",
			Self::ZipCode => "zip_code",
			Self::Country => "country",
			Self::PhoneNumber => "phone_number",
			Self::CellNumber => "cell_number",
			Self::CompanyName => "company_name",
			Self::Integer => "integer",
			Self::Float => "float",
			Self::Boolean => "boolean",
			Self::Date => "date",
			Self::DateTime => "datetime",
			Self::Time => "time",
			Self::Uuid => "uuid",
		};
		write!(f, "{}", name)
	}
}

/// Generates fake data using the specified faker type string.
///
/// # Arguments
///
/// * `faker_type` - Name of the faker type
///
/// # Returns
///
/// Returns the generated fake value as a string.
///
/// # Example
///
/// ```
/// use reinhardt_seeding::factory::generate_fake;
///
/// let name = generate_fake("name").unwrap();
/// let email = generate_fake("email").unwrap();
/// ```
pub fn generate_fake(faker_type: &str) -> SeedingResult<String> {
	let faker: FakerType = faker_type.parse()?;
	Ok(faker.generate())
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	#[case(FakerType::Name)]
	#[case(FakerType::FirstName)]
	#[case(FakerType::LastName)]
	#[case(FakerType::Email)]
	#[case(FakerType::Username)]
	#[case(FakerType::Password)]
	#[case(FakerType::Word)]
	#[case(FakerType::Sentence)]
	#[case(FakerType::City)]
	#[case(FakerType::PhoneNumber)]
	#[case(FakerType::CompanyName)]
	#[case(FakerType::Integer)]
	#[case(FakerType::Boolean)]
	#[case(FakerType::Date)]
	#[case(FakerType::Uuid)]
	fn test_faker_generate(#[case] faker_type: FakerType) {
		let value = faker_type.generate();
		assert!(
			!value.is_empty(),
			"Generated value should not be empty for {:?}",
			faker_type
		);
	}

	#[rstest]
	fn test_email_format() {
		let email = FakerType::Email.generate();
		assert!(email.contains('@'), "Email should contain @");
	}

	#[rstest]
	fn test_uuid_format() {
		let uuid_str = FakerType::Uuid.generate();
		let uuid = Uuid::parse_str(&uuid_str);
		assert!(uuid.is_ok(), "Should generate valid UUID");
	}

	#[rstest]
	fn test_date_format() {
		let date = FakerType::Date.generate();
		// Should be in YYYY-MM-DD format
		assert_eq!(date.len(), 10);
		assert_eq!(&date[4..5], "-");
		assert_eq!(&date[7..8], "-");
	}

	#[rstest]
	fn test_datetime_format() {
		let datetime = FakerType::DateTime.generate();
		// Should be in ISO 8601 format
		assert!(datetime.contains('T'));
		assert!(datetime.ends_with('Z'));
	}

	#[rstest]
	#[case("name", FakerType::Name)]
	#[case("first_name", FakerType::FirstName)]
	#[case("firstname", FakerType::FirstName)]
	#[case("email", FakerType::Email)]
	#[case("username", FakerType::Username)]
	#[case("integer", FakerType::Integer)]
	#[case("int", FakerType::Integer)]
	#[case("boolean", FakerType::Boolean)]
	#[case("bool", FakerType::Boolean)]
	#[case("uuid", FakerType::Uuid)]
	fn test_from_str(#[case] input: &str, #[case] expected: FakerType) {
		let parsed: FakerType = input.parse().unwrap();
		assert_eq!(parsed, expected);
	}

	#[rstest]
	fn test_from_str_case_insensitive() {
		let lower: FakerType = "name".parse().unwrap();
		let upper: FakerType = "NAME".parse().unwrap();
		let mixed: FakerType = "Name".parse().unwrap();
		assert_eq!(lower, upper);
		assert_eq!(lower, mixed);
	}

	#[rstest]
	fn test_from_str_unknown() {
		let result: Result<FakerType, _> = "unknown_type".parse();
		assert!(result.is_err());
	}

	#[rstest]
	fn test_generate_fake_function() {
		let name = generate_fake("name").unwrap();
		assert!(!name.is_empty());

		let result = generate_fake("invalid_type");
		assert!(result.is_err());
	}

	#[rstest]
	fn test_display() {
		assert_eq!(FakerType::Name.to_string(), "name");
		assert_eq!(FakerType::FirstName.to_string(), "first_name");
		assert_eq!(FakerType::Email.to_string(), "email");
	}

	#[rstest]
	fn test_all_names() {
		let names = FakerType::all_names();
		assert!(names.contains(&"name"));
		assert!(names.contains(&"email"));
		assert!(names.contains(&"uuid"));
	}

	#[rstest]
	fn test_generate_as_integer() {
		let value: i32 = FakerType::Integer.generate_as().unwrap();
		assert!(value >= 0 && value < 1000);
	}
}
