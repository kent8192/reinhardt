//! Command registry

use crate::BaseCommand;
use std::collections::HashMap;

/// Registry that stores and provides access to management commands by name.
pub struct CommandRegistry {
	commands: HashMap<String, Box<dyn BaseCommand>>,
}

impl CommandRegistry {
	/// Creates a new empty command registry.
	pub fn new() -> Self {
		Self {
			commands: HashMap::new(),
		}
	}

	/// Registers a command, overwriting any existing command with the same name.
	pub fn register(&mut self, command: Box<dyn BaseCommand>) {
		let name = command.name().to_string();
		self.commands.insert(name, command);
	}

	/// Returns a reference to the command with the given name, if registered.
	pub fn get(&self, name: &str) -> Option<&dyn BaseCommand> {
		self.commands.get(name).map(|cmd| &**cmd)
	}

	/// Returns a list of all registered command names.
	pub fn list(&self) -> Vec<&str> {
		self.commands.keys().map(|s| s.as_str()).collect()
	}
}

impl Default for CommandRegistry {
	fn default() -> Self {
		Self::new()
	}
}
