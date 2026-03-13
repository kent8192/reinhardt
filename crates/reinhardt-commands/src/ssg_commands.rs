//! Static site generation management command.
//!
//! Provides the `ssg_build` command for generating static HTML files
//! from registered SSG routes.

use crate::base::{BaseCommand, CommandArgument, CommandOption};
use crate::{CommandContext, CommandResult};
use async_trait::async_trait;
use reinhardt_pages::ssg::{SsgBuilder, SsgRoute};
use std::path::PathBuf;
use std::sync::Mutex;

/// Management command for building a static site.
///
/// Renders all registered SSG routes to static HTML files
/// in the specified output directory.
///
/// Routes are consumed during build, so this command can only be
/// executed once. Subsequent calls will report no routes.
pub struct SsgBuildCommand {
	/// Routes to render (wrapped in Mutex for interior mutability).
	routes: Mutex<Vec<SsgRoute>>,
}

impl SsgBuildCommand {
	/// Creates a new SSG build command with the given routes.
	pub fn new(routes: Vec<SsgRoute>) -> Self {
		Self {
			routes: Mutex::new(routes),
		}
	}
}

#[async_trait]
impl BaseCommand for SsgBuildCommand {
	fn name(&self) -> &str {
		"ssg_build"
	}

	fn description(&self) -> &str {
		"Build static HTML files from registered SSG routes"
	}

	fn help(&self) -> &str {
		"Renders all registered SSG routes to static HTML files.\n\
		 Output is written to the specified directory, mirroring URL paths.\n\
		 Optionally generates a sitemap.xml if --base-url is provided."
	}

	fn arguments(&self) -> Vec<CommandArgument> {
		vec![
			CommandArgument::optional("output_dir", "Output directory for generated files")
				.with_default("dist"),
		]
	}

	fn options(&self) -> Vec<CommandOption> {
		vec![
			CommandOption::option(Some('u'), "base-url", "Base URL for sitemap generation"),
			CommandOption::flag(None, "no-sitemap", "Disable sitemap.xml generation"),
			CommandOption::flag(Some('c'), "clean", "Clean output directory before building"),
		]
	}

	fn requires_system_checks(&self) -> bool {
		false
	}

	async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
		let output_dir = ctx
			.arg(0)
			.map(PathBuf::from)
			.unwrap_or_else(|| PathBuf::from("dist"));

		let base_url = ctx.option("base-url").cloned();
		let no_sitemap = ctx.has_option("no-sitemap");
		let clean = ctx.has_option("clean");

		// Take ownership of routes (consumes them)
		let routes = {
			let mut guard = self
				.routes
				.lock()
				.unwrap_or_else(|poisoned| poisoned.into_inner());
			std::mem::take(&mut *guard)
		};

		if routes.is_empty() {
			ctx.warning("No SSG routes registered. Nothing to build.");
			return Ok(());
		}

		ctx.info(&format!(
			"Building static site to {}...",
			output_dir.display()
		));
		ctx.info(&format!("Rendering {} route(s)...", routes.len()));

		let mut builder = SsgBuilder::new(&output_dir)
			.with_sitemap(!no_sitemap)
			.with_clean_output(clean)
			.with_routes(routes);

		if let Some(url) = base_url {
			builder = builder.with_base_url(url);
		}

		let output = builder
			.build()
			.map_err(|e| crate::CommandError::ExecutionError(e.to_string()))?;

		ctx.success(&format!(
			"Static site built: {} file(s), {} bytes total",
			output.files_written, output.total_bytes
		));

		if output.sitemap_generated {
			ctx.info("sitemap.xml generated");
		}

		for file in &output.generated_files {
			ctx.info(&format!("  {}", file.display()));
		}

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_pages::component::Page;
	use rstest::rstest;
	use tempfile::TempDir;

	fn test_page() -> Page {
		Page::text("Test Content")
	}

	#[rstest]
	fn test_ssg_build_command_name() {
		// Arrange
		let cmd = SsgBuildCommand::new(vec![]);

		// Act & Assert
		assert_eq!(cmd.name(), "ssg_build");
	}

	#[rstest]
	fn test_ssg_build_command_does_not_require_system_checks() {
		// Arrange
		let cmd = SsgBuildCommand::new(vec![]);

		// Act & Assert
		assert!(!cmd.requires_system_checks());
	}

	#[rstest]
	#[tokio::test]
	async fn test_ssg_build_command_empty_routes() {
		// Arrange
		let cmd = SsgBuildCommand::new(vec![]);
		let ctx = CommandContext::new(vec!["/tmp/ssg-test-empty".to_string()]);

		// Act
		let result = cmd.execute(&ctx).await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn test_ssg_build_command_with_routes() {
		// Arrange
		let tmpdir = TempDir::new().unwrap();
		let output_dir = tmpdir.path().join("ssg-output");

		let routes = vec![
			SsgRoute::new("/", test_page),
			SsgRoute::new("/about/", || Page::text("About")),
		];

		let cmd = SsgBuildCommand::new(routes);
		let ctx = CommandContext::new(vec![output_dir.to_string_lossy().to_string()]);

		// Act
		let result = cmd.execute(&ctx).await;

		// Assert
		assert!(result.is_ok());
		assert!(output_dir.join("index.html").exists());
		assert!(output_dir.join("about/index.html").exists());
	}

	#[rstest]
	#[tokio::test]
	async fn test_ssg_build_command_second_call_is_empty() {
		// Arrange
		let tmpdir = TempDir::new().unwrap();
		let output_dir = tmpdir.path().join("ssg-output");

		let routes = vec![SsgRoute::new("/", test_page)];
		let cmd = SsgBuildCommand::new(routes);
		let ctx = CommandContext::new(vec![output_dir.to_string_lossy().to_string()]);

		// Act - first call consumes routes
		let _ = cmd.execute(&ctx).await;

		// Act - second call should warn about no routes
		let result = cmd.execute(&ctx).await;

		// Assert
		assert!(result.is_ok());
	}
}
