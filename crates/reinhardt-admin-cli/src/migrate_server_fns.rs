//! Safe migration from explicit server-function markers to automatic registration.

mod discovery;
mod rewriter;

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;
use thiserror::Error;

use discovery::ProjectIndex;
use rewriter::{Report, ReportKind};

/// Arguments for the `migrate-server-fns` subcommand.
#[derive(Args, Debug)]
pub struct MigrateServerFnsArgs {
	/// Cargo project or workspace to inspect.
	#[arg(default_value = ".")]
	pub path: PathBuf,

	/// Write safe migrations. The default is a dry-run.
	#[arg(long)]
	pub write: bool,
}

/// Errors that prevent the migration from safely inspecting the project.
#[derive(Debug, Error)]
pub enum MigrateServerFnsError {
	/// Cargo metadata could not be loaded.
	#[error("failed to load Cargo metadata: {0}")]
	Metadata(#[from] cargo_metadata::Error),

	/// A discovered Rust source file could not be read or written.
	#[error("failed to access `{path}`: {source}")]
	Io {
		/// Source file that failed.
		path: PathBuf,
		/// Underlying filesystem error.
		source: std::io::Error,
	},

	/// A discovered Rust source file could not be parsed.
	#[error("failed to parse `{path}`: {source}")]
	Parse {
		/// Source file that failed.
		path: PathBuf,
		/// Parser error.
		source: syn::Error,
	},

	/// Cargo reported a local target outside the workspace root.
	#[error("local target `{path}` is outside workspace root `{root}`")]
	TargetOutsideWorkspace {
		/// Target source path.
		path: PathBuf,
		/// Cargo workspace root.
		root: PathBuf,
	},
}

/// Result type for server-function migrations.
pub type Result<T> = std::result::Result<T, MigrateServerFnsError>;

/// Runs the safe server-function migration.
pub fn run(args: MigrateServerFnsArgs) -> Result<()> {
	let project = ProjectIndex::discover(&args.path)?;
	let mut reports = Vec::new();
	let mut module_contexts = BTreeMap::<PathBuf, usize>::new();
	for source_module in &project.source_modules {
		*module_contexts
			.entry(source_module.path.clone())
			.or_default() += 1;
	}

	for source_module in &project.source_modules {
		let source = read_source(&source_module.path)?;
		let parsed = syn::parse_file(&source).map_err(|source| MigrateServerFnsError::Parse {
			path: source_module.path.clone(),
			source,
		})?;
		let outcome = rewriter::rewrite(
			parsed,
			&source_module.target,
			&source_module.module,
			&project.app_modules,
			&project.server_fns,
		);

		for skipped in outcome.skipped {
			reports.push(Report {
				path: source_module.relative_path.clone(),
				line: skipped.line,
				kind: skipped.kind,
			});
		}

		if outcome.rewritten.is_some() {
			if module_contexts[&source_module.path] > 1 {
				reports.push(Report {
					path: source_module.relative_path.clone(),
					line: 0,
					kind: ReportKind::IncompatibleAppOwnership,
				});
				continue;
			}
			if args.write {
				let Some(rewritten_source) = rewriter::apply_text_edits(&source, &outcome.edits)
				else {
					reports.push(Report {
						path: source_module.relative_path.clone(),
						line: 0,
						kind: ReportKind::TextEditsCouldNotBeApplied,
					});
					continue;
				};
				write_source(&source_module.path, &rewritten_source)?;
				reports.push(Report {
					path: source_module.relative_path.clone(),
					line: 0,
					kind: ReportKind::Rewrote,
				});
			} else {
				reports.push(Report {
					path: source_module.relative_path.clone(),
					line: 0,
					kind: ReportKind::WouldRewrite,
				});
			}
		}
	}

	reports.sort();
	reports.dedup();
	for report in reports {
		println!("{report}");
	}
	Ok(())
}

fn read_source(path: &Path) -> Result<String> {
	fs::read_to_string(path).map_err(|source| MigrateServerFnsError::Io {
		path: path.to_path_buf(),
		source,
	})
}

fn write_source(path: &Path, source: &str) -> Result<()> {
	let parent = path.parent().ok_or_else(|| MigrateServerFnsError::Io {
		path: path.to_path_buf(),
		source: std::io::Error::other("source path has no parent directory"),
	})?;
	let mut temporary =
		tempfile::NamedTempFile::new_in(parent).map_err(|source| MigrateServerFnsError::Io {
			path: path.to_path_buf(),
			source,
		})?;
	temporary
		.write_all(source.as_bytes())
		.map_err(|source| MigrateServerFnsError::Io {
			path: path.to_path_buf(),
			source,
		})?;
	temporary
		.persist(path)
		.map_err(|source| MigrateServerFnsError::Io {
			path: path.to_path_buf(),
			source: source.error,
		})?;
	Ok(())
}
