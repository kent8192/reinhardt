//! DI dependency graph verification tool
//!
//! This tool checks and visualizes the DI dependency graph:
//! - Show basic registry information
//! - Display dependency trees for specific types
//! - Generate Graphviz DOT format for visualization
//! - Detect circular dependencies
//!
//! Usage:
//!   cargo run --bin check-di                 # Show basic info
//!   cargo run --bin check-di -- --tree `<type>`    # Show dependency tree
//!   cargo run --bin check-di -- --dot            # Generate DOT format
//!   cargo run --bin check-di -- --check-cycles   # Check for cycles
//!   cargo run --bin check-di -- --validate       # Validate registry integrity

use reinhardt_di::graph::DependencyGraph;
use reinhardt_di::validation::{RegistryValidator, format_validation_report};
use std::env;
use std::process;
use std::sync::Arc;

fn main() {
	let args: Vec<String> = env::args().collect();

	// Get the global registry
	let registry = reinhardt_di::global_registry();

	// Count registered dependencies
	let registered_count = registry.len();

	if args.len() == 1 {
		// Basic information display
		show_basic_info(registry, registered_count);
	} else {
		match args[1].as_str() {
			"--tree" => {
				if args.len() < 3 {
					eprintln!("Error: --tree requires a type name argument");
					eprintln!("Usage: cargo run --bin check-di -- --tree <type_name>");
					process::exit(1);
				}
				show_dependency_tree(registry, &args[2]);
			}
			"--dot" => {
				generate_dot_output(registry);
			}
			"--check-cycles" => {
				check_cycles(registry);
			}
			"--validate" => {
				validate_registry(registry);
			}
			"--help" => {
				print_help();
			}
			unknown => {
				eprintln!("Unknown option: {}", unknown);
				print_help();
				process::exit(1);
			}
		}
	}
}

/// Show basic registry information
fn show_basic_info(registry: &Arc<reinhardt_di::DependencyRegistry>, registered_count: usize) {
	println!("🔍 Checking DI dependency graph...");
	println!();

	println!("✓ Found {} registered dependencies", registered_count);

	if registered_count == 0 {
		println!();
		println!("⚠️  Warning: No dependencies registered");
		println!("   Make sure to import modules that use #[injectable] or register_dependency!");
		process::exit(1);
	}

	// Check for cycles
	let graph = DependencyGraph::new(Arc::clone(registry));
	let cycles = graph.detect_cycles();

	if cycles.is_empty() {
		println!("✓ No circular dependencies detected");
	} else {
		println!("❌ Found {} circular dependenc(y/ies)", cycles.len());
		for (i, cycle) in cycles.iter().enumerate() {
			println!("   Cycle {}: {} types involved", i + 1, cycle.len());
		}
	}

	println!();

	// Show type names
	let type_names = registry.get_type_names();
	if !type_names.is_empty() {
		println!("Registered types:");
		for (i, (_type_id, type_name)) in type_names.iter().enumerate() {
			println!("  {}. {}", i + 1, type_name);
		}
	}

	println!();
	println!("✓ All checks passed");
	println!();
	println!("Note: Runtime circular dependency detection is active.");
	println!("      Any circular dependencies will be caught during resolution.");
	println!();
	println!("Run with --help to see more options.");
}

/// Show dependency tree for a specific type
fn show_dependency_tree(registry: &Arc<reinhardt_di::DependencyRegistry>, type_name: &str) {
	println!("🔍 Dependency tree for: {}", type_name);
	println!();

	// Find the type by name
	let type_names = registry.get_type_names();
	let type_id = type_names
		.iter()
		.find(|(_id, name)| name.contains(type_name))
		.map(|(id, _name)| *id);

	if let Some(type_id) = type_id {
		let graph = DependencyGraph::new(Arc::clone(registry));

		if let Some(tree) = graph.build_tree(type_id) {
			print_tree(&tree, 0);
		} else {
			println!("Unable to build dependency tree");
		}
	} else {
		println!("Type '{}' not found in registry", type_name);
		println!();
		println!("Available types:");
		for (_id, name) in type_names.iter() {
			println!("  - {}", name);
		}
		process::exit(1);
	}
}

/// Generate DOT format output
fn generate_dot_output(registry: &Arc<reinhardt_di::DependencyRegistry>) {
	let graph = DependencyGraph::new(Arc::clone(registry));
	let dot = graph.to_dot();
	println!("{}", dot);
	eprintln!();
	eprintln!("To generate a PNG image, save this output to a file and run:");
	eprintln!("  cargo run --bin check-di -- --dot > dependencies.dot");
	eprintln!("  dot -Tpng dependencies.dot -o dependencies.png");
}

/// Check for circular dependencies
fn check_cycles(registry: &Arc<reinhardt_di::DependencyRegistry>) {
	println!("🔍 Checking for circular dependencies...");
	println!();

	let graph = DependencyGraph::new(Arc::clone(registry));
	let cycles = graph.detect_cycles();

	if cycles.is_empty() {
		println!("✓ No circular dependencies detected");
		println!();
	} else {
		println!("❌ Found {} circular dependenc(y/ies):", cycles.len());
		println!();

		let type_names = registry.get_type_names();
		for (i, cycle) in cycles.iter().enumerate() {
			println!("Cycle {}:", i + 1);
			for type_id in cycle {
				let name = type_names.get(type_id).copied().unwrap_or("Unknown");
				println!("  → {}", name);
			}
			println!();
		}
		process::exit(1);
	}
}

/// Print dependency tree recursively
fn print_tree(node: &reinhardt_di::graph::DependencyNode, depth: usize) {
	let indent = "  ".repeat(depth);
	println!("{}{}", indent, node.type_name);

	for child in &node.dependencies {
		print_tree(child, depth + 1);
	}
}

/// Validate the registry for missing deps, scope issues, and cycles
fn validate_registry(registry: &Arc<reinhardt_di::DependencyRegistry>) {
	println!("Validating DI registry...");
	println!();

	let validator = RegistryValidator::new(Arc::clone(registry));
	match validator.validate() {
		Ok(()) => {
			println!("DI registry validation passed: no issues found");
		}
		Err(errors) => {
			eprint!("{}", format_validation_report(&errors));
			process::exit(1);
		}
	}
}

/// Print help message
fn print_help() {
	println!("DI Dependency Graph Verification Tool");
	println!();
	println!("Usage:");
	println!("  cargo run --bin check-di              # Show basic info");
	println!("  cargo run --bin check-di -- --tree <type_name>  # Show dependency tree");
	println!("  cargo run --bin check-di -- --dot              # Generate DOT format");
	println!("  cargo run --bin check-di -- --check-cycles     # Check for cycles");
	println!("  cargo run --bin check-di -- --validate         # Validate registry integrity");
	println!("  cargo run --bin check-di -- --help             # Show this help");
	println!();
	println!("Examples:");
	println!("  cargo run --bin check-di -- --tree MyService");
	println!("  cargo run --bin check-di -- --dot > deps.dot && dot -Tpng deps.dot -o deps.png");
}
