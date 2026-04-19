//! createsuperuser CLI command
//!
//! Creates a superuser account interactively.
//!
//! **Deprecated**: Use the management command instead:
//! ```bash
//! cargo run --bin manage createsuperuser
//! ```
//! The management command supports custom user models via
//! [`reinhardt_auth::register_superuser_creator`].

use clap::Parser;
use console::style;
use dialoguer::{Confirm, Input, Password};

#[cfg(feature = "database")]
use argon2::Argon2;

#[cfg(feature = "database")]
use password_hash::{PasswordHasher, SaltString, rand_core::OsRng};

#[cfg(feature = "database")]
use chrono::Utc;

#[cfg(feature = "database")]
use reinhardt_core::macros::model;

#[cfg(feature = "database")]
use reinhardt_db::{DatabaseConnection, prelude::Model};

#[derive(Parser, Debug)]
#[command(name = "createsuperuser")]
#[command(about = "Creates a superuser account", long_about = None)]
struct Args {
	/// Username for the superuser
	#[arg(long, value_name = "USERNAME")]
	username: Option<String>,

	/// Email address for the superuser
	#[arg(long, value_name = "EMAIL")]
	email: Option<String>,

	/// Skip the password prompt (use with caution)
	#[arg(long)]
	no_password: bool,

	/// Non-interactive mode (requires --username and --email)
	#[arg(long)]
	noinput: bool,

	/// Database connection string
	#[arg(long, value_name = "DATABASE", default_value = "sqlite::memory:")]
	database: String,
}

#[cfg(feature = "database")]
#[model(table_name = "auth_user")]
#[derive(serde::Serialize, serde::Deserialize)]
/// Authentication user model for the createsuperuser command.
pub struct AuthUser {
	/// Primary key identifier.
	#[field(primary_key = true)]
	pub id: Option<i32>,
	/// The user's login name.
	#[field(max_length = 150)]
	pub username: String,
	/// The user's email address.
	#[field(max_length = 254)]
	pub email: String,
	/// The hashed password.
	#[field(max_length = 255)]
	pub password_hash: Option<String>,
	/// Whether the user is a staff member.
	pub is_staff: bool,
	/// Whether the user account is active.
	pub is_active: bool,
	/// Whether the user has superuser privileges.
	pub is_superuser: bool,
	/// When the user account was created.
	pub date_joined: chrono::DateTime<Utc>,
}

fn validate_email(email: &str) -> bool {
	email.contains('@') && email.contains('.')
}

fn validate_username(username: &str) -> bool {
	!username.is_empty() && username.len() >= 3
}

#[cfg(feature = "database")]
async fn create_user_in_database(
	username: &str,
	email: &str,
	password: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
	// Hash the password if provided
	let password_hash = if let Some(pwd) = password {
		let salt = SaltString::generate(&mut OsRng);
		let argon2 = Argon2::default();
		let hash = argon2
			.hash_password(pwd.as_bytes(), &salt)
			.map_err(|e| format!("Failed to hash password: {}", e))?;
		Some(hash.to_string())
	} else {
		None
	};

	// Create user with ORM
	let user = AuthUser {
		id: None,
		username: username.to_string(),
		email: email.to_string(),
		password_hash,
		is_staff: true,
		is_superuser: true,
		is_active: true,
		date_joined: Utc::now(),
	};

	let manager = AuthUser::objects();
	manager.create(&user).await?;

	Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let args = Args::parse();

	println!("{}", style("Creating superuser account").cyan().bold());
	println!();

	// Get username
	let username = if let Some(username) = args.username {
		if args.noinput && !validate_username(&username) {
			eprintln!(
				"{}",
				style("Error: Username must be at least 3 characters").red()
			);
			std::process::exit(1);
		}
		username
	} else if args.noinput {
		eprintln!(
			"{}",
			style("Error: --username is required in non-interactive mode").red()
		);
		std::process::exit(1);
	} else {
		Input::<String>::new()
			.with_prompt("Username")
			.validate_with(|input: &String| -> Result<(), &str> {
				if validate_username(input) {
					Ok(())
				} else {
					Err("Username must be at least 3 characters")
				}
			})
			.interact_text()?
	};

	// Get email
	let email = if let Some(email) = args.email {
		if args.noinput && !validate_email(&email) {
			eprintln!("{}", style("Error: Invalid email address").red());
			std::process::exit(1);
		}
		email
	} else if args.noinput {
		eprintln!(
			"{}",
			style("Error: --email is required in non-interactive mode").red()
		);
		std::process::exit(1);
	} else {
		Input::<String>::new()
			.with_prompt("Email address")
			.validate_with(|input: &String| -> Result<(), &str> {
				if validate_email(input) {
					Ok(())
				} else {
					Err("Invalid email address")
				}
			})
			.interact_text()?
	};

	// Get password
	let password = if args.no_password {
		println!(
			"{}",
			style("Warning: Superuser created without password").yellow()
		);
		None
	} else if args.noinput {
		eprintln!(
			"{}",
			style("Error: Cannot set password in non-interactive mode without --no-password").red()
		);
		std::process::exit(1);
	} else {
		let password = Password::new()
			.with_prompt("Password")
			.with_confirmation("Password (again)", "Error: Passwords do not match")
			.validate_with(|input: &String| -> Result<(), &str> {
				if input.len() >= 8 {
					Ok(())
				} else {
					Err("Password must be at least 8 characters")
				}
			})
			.interact()?;
		Some(password)
	};

	println!();
	println!("{}", style("Superuser details:").green().bold());
	println!("  Username: {}", style(&username).yellow());
	println!("  Email:    {}", style(&email).yellow());
	if password.is_some() {
		println!("  Password: {}", style("(set)").green());
	} else {
		println!("  Password: {}", style("(not set)").red());
	}

	// Confirmation
	if !args.noinput {
		println!();
		let confirmed = Confirm::new()
			.with_prompt("Create superuser?")
			.default(true)
			.interact()?;

		if !confirmed {
			println!("{}", style("Superuser creation cancelled").yellow());
			return Ok(());
		}
	}

	// Create the user in the database
	println!();
	println!("{}", style("Creating user in database...").cyan());

	#[cfg(feature = "database")]
	{
		match create_database_user(&args.database, &username, &email, password.as_deref()).await {
			Ok(_) => {
				println!(
					"{}",
					style("✓ Superuser created successfully!").green().bold()
				);
				println!();
				println!("  Database: {}", style(&args.database).dim());
				println!("  Username: {}", style(&username).yellow());
				println!("  Email:    {}", style(&email).yellow());
			}
			Err(e) => {
				eprintln!("{}", style(format!("Error: {}", e)).red().bold());
				std::process::exit(1);
			}
		}
	}

	#[cfg(not(feature = "database"))]
	{
		println!();
		println!(
			"{}",
			style("✓ Superuser validation completed!").green().bold()
		);
		println!();
		println!("{}", style("Note: Database feature not enabled").yellow());
		println!(
			"{}",
			style("Rebuild with --features database to enable database integration").dim()
		);
		println!();
		println!("  Username: {}", style(&username).yellow());
		println!("  Email:    {}", style(&email).yellow());
	}

	Ok(())
}

#[cfg(feature = "database")]
async fn create_database_user(
	database_url: &str,
	username: &str,
	email: &str,
	password: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
	// Create database connection to initialize the ORM context
	let _connection = DatabaseConnection::connect(database_url).await?;

	// Create user
	create_user_in_database(username, email, password).await?;

	Ok(())
}
