//! Plugin capability system.
//!
//! Capabilities define what functionality a plugin can provide or consume.
//! Plugins must declare their capabilities, and only declared capabilities
//! are activated at runtime.
//!
//! # Design
//!
//! The capability system uses a two-tier approach:
//! - [`PluginCapability`]: Core framework capabilities (compile-time optimized)
//! - [`Capability`]: Wrapper supporting both core and custom capabilities
//!
//! # Example
//!
//! ```ignore
//! use reinhardt_dentdelion::capability::{Capability, PluginCapability};
//!
//! // Core capabilities
//! let middleware = Capability::Core(PluginCapability::Middleware);
//! let models = Capability::Core(PluginCapability::Models);
//!
//! // Custom capability
//! let custom = Capability::Custom("my-custom-feature".to_string());
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;

/// Core plugin capabilities defined by the framework.
///
/// These are the standard capabilities that plugins can provide.
/// Using an enum for core capabilities provides:
/// - Compile-time type safety
/// - Efficient storage and comparison
/// - Pattern matching support
///
/// The `#[non_exhaustive]` attribute allows adding new capabilities
/// in future versions without breaking existing code.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PluginCapability {
	/// Provides HTTP middleware components.
	///
	/// Plugins with this capability can intercept and modify
	/// HTTP requests and responses.
	Middleware,

	/// Provides database models and migrations.
	///
	/// Plugins with this capability can define database tables
	/// and manage their schema through migrations.
	///
	/// **Note**: Only available for static plugins (Rust crates).
	/// WASM plugins cannot provide models due to compile-time requirements.
	Models,

	/// Provides CLI management commands.
	///
	/// Plugins with this capability can add commands to
	/// `reinhardt-admin-cli`.
	Commands,

	/// Provides REST API ViewSets.
	///
	/// Plugins with this capability can register ViewSets
	/// that handle REST API endpoints.
	ViewSets,

	/// Provides custom signals.
	///
	/// Plugins with this capability can define and emit
	/// custom signals, or subscribe to existing signals.
	Signals,

	/// Provides DI services.
	///
	/// Plugins with this capability can register services
	/// in the dependency injection container.
	Services,

	/// Provides authentication backends.
	///
	/// Plugins with this capability can implement authentication
	/// mechanisms (JWT, OAuth, etc.).
	Auth,

	/// Provides template engines or filters.
	///
	/// Plugins with this capability can extend the template
	/// rendering system.
	Templates,

	/// Provides static file handling.
	///
	/// Plugins with this capability can serve or process
	/// static files.
	StaticFiles,

	/// Provides URL routing.
	///
	/// Plugins with this capability can register custom
	/// routes and route handlers.
	Routing,

	/// Provides signal receivers.
	///
	/// Plugins with this capability can listen to and
	/// respond to signals from other parts of the system.
	SignalReceivers,

	/// Provides HTTP handlers/views.
	///
	/// Plugins with this capability can handle HTTP requests
	/// directly (not through ViewSets).
	Handlers,

	/// Provides network/HTTP access.
	///
	/// Plugins with this capability can make external HTTP requests
	/// via the host API.
	NetworkAccess,

	/// Provides database access.
	///
	/// Plugins with this capability can execute SQL queries and
	/// statements via the host API.
	DatabaseAccess,

	/// Provides static site generation.
	///
	/// Plugins with this capability can generate static HTML pages
	/// from routes and components at build time.
	///
	/// **Note**: Only available for static plugins (Rust crates).
	/// WASM plugins cannot provide SSG due to compile-time requirements
	/// for route introspection and component rendering.
	StaticSiteGeneration,

	// ==========================================================================
	// Frontend Integration Capabilities (for react-delion, vue-delion, etc.)
	// ==========================================================================
	/// Provides server-side rendering (SSR) for frontend frameworks.
	///
	/// Plugins with this capability can render React/Vue components
	/// to HTML on the server using deno_core (V8).
	///
	/// **Note**: Requires TypeScript runtime (`ts` feature).
	FrontendSsr,

	/// Provides client-side hydration support.
	///
	/// Plugins with this capability can generate hydration scripts
	/// and manage client-side state restoration after SSR.
	FrontendHydration,

	/// Provides TypeScript/JavaScript runtime execution.
	///
	/// Plugins with this capability can execute TypeScript code
	/// directly via deno_core (V8 engine) without transpilation.
	///
	/// **Note**: Requires `ts` feature.
	TypeScriptRuntime,

	/// Provides build tool integration.
	///
	/// Plugins with this capability can integrate with frontend
	/// build tools (Vite, Rspack, webpack, Farm, etc.).
	BuildToolIntegration,

	/// Provides hot module replacement (HMR) support.
	///
	/// Plugins with this capability can dynamically reload
	/// modules during development without full page refresh.
	HotModuleReplacement,
}

impl PluginCapability {
	/// Returns all core capabilities.
	pub fn all() -> &'static [Self] {
		&[
			Self::Middleware,
			Self::Models,
			Self::Commands,
			Self::ViewSets,
			Self::Signals,
			Self::Services,
			Self::Auth,
			Self::Templates,
			Self::StaticFiles,
			Self::Routing,
			Self::SignalReceivers,
			Self::Handlers,
			Self::NetworkAccess,
			Self::DatabaseAccess,
			Self::StaticSiteGeneration,
			Self::FrontendSsr,
			Self::FrontendHydration,
			Self::TypeScriptRuntime,
			Self::BuildToolIntegration,
			Self::HotModuleReplacement,
		]
	}

	/// Returns the string identifier for this capability.
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Middleware => "middleware",
			Self::Models => "models",
			Self::Commands => "commands",
			Self::ViewSets => "viewsets",
			Self::Signals => "signals",
			Self::Services => "services",
			Self::Auth => "auth",
			Self::Templates => "templates",
			Self::StaticFiles => "static_files",
			Self::Routing => "routing",
			Self::SignalReceivers => "signal_receivers",
			Self::Handlers => "handlers",
			Self::NetworkAccess => "network_access",
			Self::DatabaseAccess => "database_access",
			Self::StaticSiteGeneration => "static_site_generation",
			Self::FrontendSsr => "frontend_ssr",
			Self::FrontendHydration => "frontend_hydration",
			Self::TypeScriptRuntime => "typescript_runtime",
			Self::BuildToolIntegration => "build_tool_integration",
			Self::HotModuleReplacement => "hot_module_replacement",
		}
	}

	/// Returns whether this capability is available for WASM plugins.
	///
	/// Some capabilities require compile-time integration and are
	/// therefore not available for dynamic (WASM) plugins.
	pub fn is_wasm_compatible(&self) -> bool {
		match self {
			// Models require compile-time linkme registration
			Self::Models => false,
			// SSG requires compile-time route introspection and component rendering
			Self::StaticSiteGeneration => false,
			// Frontend SSR/Hydration require TypeScript runtime (deno_core)
			Self::FrontendSsr | Self::FrontendHydration => false,
			// TypeScript runtime requires deno_core, not WASM
			Self::TypeScriptRuntime => false,
			// HMR requires TypeScript runtime for dynamic module reload
			Self::HotModuleReplacement => false,
			// All other capabilities can be provided by WASM plugins
			_ => true,
		}
	}

	/// Returns whether this capability requires TypeScript runtime.
	pub fn requires_ts_runtime(&self) -> bool {
		matches!(
			self,
			Self::FrontendSsr
				| Self::FrontendHydration
				| Self::TypeScriptRuntime
				| Self::HotModuleReplacement
		)
	}
}

impl fmt::Display for PluginCapability {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

impl std::str::FromStr for PluginCapability {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s.to_lowercase().as_str() {
			"middleware" => Ok(Self::Middleware),
			"models" => Ok(Self::Models),
			"commands" => Ok(Self::Commands),
			"viewsets" => Ok(Self::ViewSets),
			"signals" => Ok(Self::Signals),
			"services" => Ok(Self::Services),
			"auth" => Ok(Self::Auth),
			"templates" => Ok(Self::Templates),
			"static_files" | "staticfiles" => Ok(Self::StaticFiles),
			"routing" => Ok(Self::Routing),
			"signal_receivers" | "signalreceivers" => Ok(Self::SignalReceivers),
			"handlers" => Ok(Self::Handlers),
			"network_access" | "networkaccess" => Ok(Self::NetworkAccess),
			"database_access" | "databaseaccess" => Ok(Self::DatabaseAccess),
			"static_site_generation" | "staticsitegeneration" | "ssg" => {
				Ok(Self::StaticSiteGeneration)
			}
			"frontend_ssr" | "frontendssr" | "ssr" => Ok(Self::FrontendSsr),
			"frontend_hydration" | "frontendhydration" | "hydration" => Ok(Self::FrontendHydration),
			"typescript_runtime" | "typescriptruntime" | "ts_runtime" | "tsruntime" => {
				Ok(Self::TypeScriptRuntime)
			}
			"build_tool_integration" | "buildtoolintegration" => Ok(Self::BuildToolIntegration),
			"hot_module_replacement" | "hotmodulereplacment" | "hmr" => {
				Ok(Self::HotModuleReplacement)
			}
			_ => Err(format!("unknown capability: {s}")),
		}
	}
}

/// Extended capability wrapper supporting custom capabilities.
///
/// This allows third-party plugins to define custom capabilities
/// while maintaining efficiency for core capabilities.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Capability {
	/// Core framework capability.
	Core(PluginCapability),
	/// Custom capability defined by third-party plugins.
	Custom(String),
}

impl Capability {
	/// Creates a new core capability.
	pub fn core(capability: PluginCapability) -> Self {
		Self::Core(capability)
	}

	/// Creates a new custom capability.
	pub fn custom(name: impl Into<String>) -> Self {
		Self::Custom(name.into())
	}

	/// Returns the string identifier for this capability.
	pub fn as_str(&self) -> &str {
		match self {
			Self::Core(cap) => cap.as_str(),
			Self::Custom(name) => name.as_str(),
		}
	}

	/// Returns whether this is a core capability.
	pub fn is_core(&self) -> bool {
		matches!(self, Self::Core(_))
	}

	/// Returns whether this capability is available for WASM plugins.
	pub fn is_wasm_compatible(&self) -> bool {
		match self {
			Self::Core(cap) => cap.is_wasm_compatible(),
			// Custom capabilities are assumed to be WASM-compatible
			// unless explicitly stated otherwise
			Self::Custom(_) => true,
		}
	}
}

impl fmt::Display for Capability {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

impl From<PluginCapability> for Capability {
	fn from(cap: PluginCapability) -> Self {
		Self::Core(cap)
	}
}

impl std::str::FromStr for Capability {
	type Err = std::convert::Infallible;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		// Try to parse as core capability first
		if let Ok(core) = s.parse::<PluginCapability>() {
			Ok(Self::Core(core))
		} else {
			// Treat as custom capability
			Ok(Self::Custom(s.to_string()))
		}
	}
}

// =============================================================================
// Plugin Tier and Trust Level
// =============================================================================

/// Plugin resource tier determining runtime limits.
///
/// Tiers allow plugins to request different resource allocations based on
/// their needs. Higher tiers provide more resources but may require
/// additional verification or trust.
///
/// # Example
///
/// ```ignore
/// use reinhardt_dentdelion::capability::PluginTier;
///
/// let tier = PluginTier::Premium;
/// let limits = tier.limits();
/// println!("Memory limit: {} MB", limits.memory_limit_bytes / 1024 / 1024);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PluginTier {
	/// Standard tier with default resource limits.
	///
	/// - Memory: 128 MB
	/// - Timeout: 30 seconds
	/// - Fuel: 100M instructions
	#[default]
	Standard,

	/// Premium tier with extended resource limits.
	///
	/// - Memory: 512 MB
	/// - Timeout: 60 seconds
	/// - Fuel: 500M instructions
	Premium,

	/// Enterprise tier with maximum resource limits.
	///
	/// - Memory: 1 GB
	/// - Timeout: 120 seconds
	/// - Fuel: 1B instructions
	Enterprise,
}

impl PluginTier {
	/// Returns all available tiers.
	pub fn all() -> &'static [Self] {
		&[Self::Standard, Self::Premium, Self::Enterprise]
	}

	/// Returns the string identifier for this tier.
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Standard => "standard",
			Self::Premium => "premium",
			Self::Enterprise => "enterprise",
		}
	}

	/// Returns the memory limit in bytes for this tier.
	pub fn memory_limit_bytes(&self) -> usize {
		match self {
			Self::Standard => 128 * 1024 * 1024,    // 128 MB
			Self::Premium => 512 * 1024 * 1024,     // 512 MB
			Self::Enterprise => 1024 * 1024 * 1024, // 1 GB
		}
	}

	/// Returns the execution timeout in seconds for this tier.
	pub fn timeout_secs(&self) -> u64 {
		match self {
			Self::Standard => 30,
			Self::Premium => 60,
			Self::Enterprise => 120,
		}
	}

	/// Returns the fuel limit (CPU instructions) for this tier.
	pub fn fuel_limit(&self) -> u64 {
		match self {
			Self::Standard => 100_000_000,     // 100M
			Self::Premium => 500_000_000,      // 500M
			Self::Enterprise => 1_000_000_000, // 1B
		}
	}
}

impl fmt::Display for PluginTier {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

impl std::str::FromStr for PluginTier {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s.to_lowercase().as_str() {
			"standard" | "std" => Ok(Self::Standard),
			"premium" | "pro" => Ok(Self::Premium),
			"enterprise" | "ent" => Ok(Self::Enterprise),
			_ => Err(format!("unknown plugin tier: {s}")),
		}
	}
}

/// Plugin trust level determining security restrictions.
///
/// Trust levels control what capabilities a plugin can access and how
/// strictly it is sandboxed. Higher trust levels reduce restrictions
/// but increase potential security risks.
///
/// # Security Considerations
///
/// - `Untrusted`: Safe for third-party plugins from unknown sources
/// - `Verified`: For plugins from known sources with signature verification
/// - `Trusted`: Only for first-party plugins (effectively unsandboxed)
///
/// # Example
///
/// ```ignore
/// use reinhardt_dentdelion::capability::TrustLevel;
///
/// let trust = TrustLevel::Verified;
/// if trust.allows_network() {
///     // Plugin can make network requests
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
	/// Untrusted plugin with strict sandboxing.
	///
	/// - No network access (unless explicitly granted via capability)
	/// - No database access (unless explicitly granted via capability)
	/// - No filesystem access
	/// - All operations are strictly sandboxed
	#[default]
	Untrusted,

	/// Verified plugin with relaxed restrictions.
	///
	/// - Network access allowed (with capability)
	/// - Database access allowed (with capability)
	/// - No filesystem access
	/// - Must pass signature verification
	Verified,

	/// Fully trusted plugin with minimal restrictions.
	///
	/// **Warning**: This level should only be used for first-party plugins.
	///
	/// - Full network access
	/// - Full database access
	/// - Read-only filesystem access to plugin directory
	/// - Runs with host-level privileges
	Trusted,
}

impl TrustLevel {
	/// Returns all available trust levels.
	pub fn all() -> &'static [Self] {
		&[Self::Untrusted, Self::Verified, Self::Trusted]
	}

	/// Returns the string identifier for this trust level.
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Untrusted => "untrusted",
			Self::Verified => "verified",
			Self::Trusted => "trusted",
		}
	}

	/// Returns whether this trust level allows network access.
	///
	/// Note: Even if allowed, the plugin must also have the `NetworkAccess`
	/// capability to actually make network requests.
	pub fn allows_network(&self) -> bool {
		!matches!(self, Self::Untrusted)
	}

	/// Returns whether this trust level allows database access.
	///
	/// Note: Even if allowed, the plugin must also have the `DatabaseAccess`
	/// capability to actually execute queries.
	pub fn allows_database(&self) -> bool {
		!matches!(self, Self::Untrusted)
	}

	/// Returns whether this trust level allows filesystem access.
	///
	/// Only `Trusted` plugins can access the filesystem.
	pub fn allows_filesystem(&self) -> bool {
		matches!(self, Self::Trusted)
	}

	/// Returns whether this trust level requires signature verification.
	pub fn requires_verification(&self) -> bool {
		matches!(self, Self::Verified)
	}

	/// Returns whether this trust level is considered safe for third-party plugins.
	pub fn is_safe_for_third_party(&self) -> bool {
		matches!(self, Self::Untrusted | Self::Verified)
	}

	/// Returns whether this trust level allows arbitrary JavaScript execution.
	///
	/// Only `Trusted` plugins can execute arbitrary JavaScript code via `eval_js`,
	/// as this provides unrestricted access to the JavaScript runtime.
	pub fn allows_js_execution(&self) -> bool {
		matches!(self, Self::Trusted)
	}

	/// Returns whether this trust level allows server-side rendering.
	///
	/// `Verified` and `Trusted` plugins can render components via SSR.
	/// `Untrusted` plugins cannot use SSR as it involves JavaScript execution.
	pub fn allows_ssr(&self) -> bool {
		!matches!(self, Self::Untrusted)
	}
}

impl fmt::Display for TrustLevel {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

impl std::str::FromStr for TrustLevel {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s.to_lowercase().as_str() {
			"untrusted" | "sandbox" | "sandboxed" => Ok(Self::Untrusted),
			"verified" | "signed" => Ok(Self::Verified),
			"trusted" | "full" => Ok(Self::Trusted),
			_ => Err(format!("unknown trust level: {s}")),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_plugin_capability_display() {
		assert_eq!(PluginCapability::Middleware.to_string(), "middleware");
		assert_eq!(PluginCapability::Models.to_string(), "models");
		assert_eq!(PluginCapability::StaticFiles.to_string(), "static_files");
	}

	#[test]
	fn test_plugin_capability_from_str() {
		assert_eq!(
			"middleware".parse::<PluginCapability>().unwrap(),
			PluginCapability::Middleware
		);
		assert_eq!(
			"MODELS".parse::<PluginCapability>().unwrap(),
			PluginCapability::Models
		);
		assert!("unknown".parse::<PluginCapability>().is_err());
	}

	#[test]
	fn test_capability_from_str() {
		assert_eq!(
			"middleware".parse::<Capability>().unwrap(),
			Capability::Core(PluginCapability::Middleware)
		);
		assert_eq!(
			"custom-feature".parse::<Capability>().unwrap(),
			Capability::Custom("custom-feature".to_string())
		);
	}

	#[test]
	fn test_wasm_compatibility() {
		assert!(PluginCapability::Middleware.is_wasm_compatible());
		assert!(PluginCapability::Commands.is_wasm_compatible());
		assert!(!PluginCapability::Models.is_wasm_compatible());
		assert!(!PluginCapability::StaticSiteGeneration.is_wasm_compatible());
	}

	#[test]
	fn test_static_site_generation_capability() {
		assert_eq!(
			PluginCapability::StaticSiteGeneration.to_string(),
			"static_site_generation"
		);
		assert_eq!(
			"static_site_generation"
				.parse::<PluginCapability>()
				.unwrap(),
			PluginCapability::StaticSiteGeneration
		);
		assert_eq!(
			"ssg".parse::<PluginCapability>().unwrap(),
			PluginCapability::StaticSiteGeneration
		);
	}

	#[test]
	fn test_frontend_capabilities() {
		// FrontendSsr
		assert_eq!(PluginCapability::FrontendSsr.to_string(), "frontend_ssr");
		assert_eq!(
			"frontend_ssr".parse::<PluginCapability>().unwrap(),
			PluginCapability::FrontendSsr
		);
		assert_eq!(
			"ssr".parse::<PluginCapability>().unwrap(),
			PluginCapability::FrontendSsr
		);

		// FrontendHydration
		assert_eq!(
			PluginCapability::FrontendHydration.to_string(),
			"frontend_hydration"
		);
		assert_eq!(
			"hydration".parse::<PluginCapability>().unwrap(),
			PluginCapability::FrontendHydration
		);

		// TypeScriptRuntime
		assert_eq!(
			PluginCapability::TypeScriptRuntime.to_string(),
			"typescript_runtime"
		);
		assert_eq!(
			"ts_runtime".parse::<PluginCapability>().unwrap(),
			PluginCapability::TypeScriptRuntime
		);

		// BuildToolIntegration
		assert_eq!(
			PluginCapability::BuildToolIntegration.to_string(),
			"build_tool_integration"
		);

		// HotModuleReplacement
		assert_eq!(
			PluginCapability::HotModuleReplacement.to_string(),
			"hot_module_replacement"
		);
		assert_eq!(
			"hmr".parse::<PluginCapability>().unwrap(),
			PluginCapability::HotModuleReplacement
		);
	}

	#[test]
	fn test_frontend_capabilities_wasm_compatibility() {
		// Frontend capabilities require TypeScript runtime, not WASM
		assert!(!PluginCapability::FrontendSsr.is_wasm_compatible());
		assert!(!PluginCapability::FrontendHydration.is_wasm_compatible());
		assert!(!PluginCapability::TypeScriptRuntime.is_wasm_compatible());
		assert!(!PluginCapability::HotModuleReplacement.is_wasm_compatible());

		// BuildToolIntegration can be used with WASM
		assert!(PluginCapability::BuildToolIntegration.is_wasm_compatible());
	}

	#[test]
	fn test_requires_ts_runtime() {
		assert!(PluginCapability::FrontendSsr.requires_ts_runtime());
		assert!(PluginCapability::FrontendHydration.requires_ts_runtime());
		assert!(PluginCapability::TypeScriptRuntime.requires_ts_runtime());
		assert!(PluginCapability::HotModuleReplacement.requires_ts_runtime());

		// Non-frontend capabilities don't require TS runtime
		assert!(!PluginCapability::Middleware.requires_ts_runtime());
		assert!(!PluginCapability::BuildToolIntegration.requires_ts_runtime());
	}

	// =========================================================================
	// PluginTier Tests
	// =========================================================================

	#[test]
	fn test_plugin_tier_default() {
		assert_eq!(PluginTier::default(), PluginTier::Standard);
	}

	#[test]
	fn test_plugin_tier_display() {
		assert_eq!(PluginTier::Standard.to_string(), "standard");
		assert_eq!(PluginTier::Premium.to_string(), "premium");
		assert_eq!(PluginTier::Enterprise.to_string(), "enterprise");
	}

	#[test]
	fn test_plugin_tier_from_str() {
		assert_eq!(
			"standard".parse::<PluginTier>().unwrap(),
			PluginTier::Standard
		);
		assert_eq!("std".parse::<PluginTier>().unwrap(), PluginTier::Standard);
		assert_eq!(
			"premium".parse::<PluginTier>().unwrap(),
			PluginTier::Premium
		);
		assert_eq!("pro".parse::<PluginTier>().unwrap(), PluginTier::Premium);
		assert_eq!(
			"enterprise".parse::<PluginTier>().unwrap(),
			PluginTier::Enterprise
		);
		assert_eq!("ent".parse::<PluginTier>().unwrap(), PluginTier::Enterprise);
		assert!("unknown".parse::<PluginTier>().is_err());
	}

	#[test]
	fn test_plugin_tier_memory_limits() {
		assert_eq!(PluginTier::Standard.memory_limit_bytes(), 128 * 1024 * 1024);
		assert_eq!(PluginTier::Premium.memory_limit_bytes(), 512 * 1024 * 1024);
		assert_eq!(
			PluginTier::Enterprise.memory_limit_bytes(),
			1024 * 1024 * 1024
		);
	}

	#[test]
	fn test_plugin_tier_timeout() {
		assert_eq!(PluginTier::Standard.timeout_secs(), 30);
		assert_eq!(PluginTier::Premium.timeout_secs(), 60);
		assert_eq!(PluginTier::Enterprise.timeout_secs(), 120);
	}

	#[test]
	fn test_plugin_tier_fuel_limits() {
		assert_eq!(PluginTier::Standard.fuel_limit(), 100_000_000);
		assert_eq!(PluginTier::Premium.fuel_limit(), 500_000_000);
		assert_eq!(PluginTier::Enterprise.fuel_limit(), 1_000_000_000);
	}

	#[test]
	fn test_plugin_tier_all() {
		let all = PluginTier::all();
		assert_eq!(all.len(), 3);
		assert!(all.contains(&PluginTier::Standard));
		assert!(all.contains(&PluginTier::Premium));
		assert!(all.contains(&PluginTier::Enterprise));
	}

	// =========================================================================
	// TrustLevel Tests
	// =========================================================================

	#[test]
	fn test_trust_level_default() {
		assert_eq!(TrustLevel::default(), TrustLevel::Untrusted);
	}

	#[test]
	fn test_trust_level_display() {
		assert_eq!(TrustLevel::Untrusted.to_string(), "untrusted");
		assert_eq!(TrustLevel::Verified.to_string(), "verified");
		assert_eq!(TrustLevel::Trusted.to_string(), "trusted");
	}

	#[test]
	fn test_trust_level_from_str() {
		assert_eq!(
			"untrusted".parse::<TrustLevel>().unwrap(),
			TrustLevel::Untrusted
		);
		assert_eq!(
			"sandbox".parse::<TrustLevel>().unwrap(),
			TrustLevel::Untrusted
		);
		assert_eq!(
			"verified".parse::<TrustLevel>().unwrap(),
			TrustLevel::Verified
		);
		assert_eq!(
			"signed".parse::<TrustLevel>().unwrap(),
			TrustLevel::Verified
		);
		assert_eq!(
			"trusted".parse::<TrustLevel>().unwrap(),
			TrustLevel::Trusted
		);
		assert_eq!("full".parse::<TrustLevel>().unwrap(), TrustLevel::Trusted);
		assert!("unknown".parse::<TrustLevel>().is_err());
	}

	#[test]
	fn test_trust_level_network_access() {
		assert!(!TrustLevel::Untrusted.allows_network());
		assert!(TrustLevel::Verified.allows_network());
		assert!(TrustLevel::Trusted.allows_network());
	}

	#[test]
	fn test_trust_level_database_access() {
		assert!(!TrustLevel::Untrusted.allows_database());
		assert!(TrustLevel::Verified.allows_database());
		assert!(TrustLevel::Trusted.allows_database());
	}

	#[test]
	fn test_trust_level_filesystem_access() {
		assert!(!TrustLevel::Untrusted.allows_filesystem());
		assert!(!TrustLevel::Verified.allows_filesystem());
		assert!(TrustLevel::Trusted.allows_filesystem());
	}

	#[test]
	fn test_trust_level_verification_requirement() {
		assert!(!TrustLevel::Untrusted.requires_verification());
		assert!(TrustLevel::Verified.requires_verification());
		assert!(!TrustLevel::Trusted.requires_verification());
	}

	#[test]
	fn test_trust_level_third_party_safety() {
		assert!(TrustLevel::Untrusted.is_safe_for_third_party());
		assert!(TrustLevel::Verified.is_safe_for_third_party());
		assert!(!TrustLevel::Trusted.is_safe_for_third_party());
	}

	#[test]
	fn test_trust_level_all() {
		let all = TrustLevel::all();
		assert_eq!(all.len(), 3);
		assert!(all.contains(&TrustLevel::Untrusted));
		assert!(all.contains(&TrustLevel::Verified));
		assert!(all.contains(&TrustLevel::Trusted));
	}

	#[test]
	fn test_trust_level_js_execution() {
		assert!(!TrustLevel::Untrusted.allows_js_execution());
		assert!(!TrustLevel::Verified.allows_js_execution());
		assert!(TrustLevel::Trusted.allows_js_execution());
	}

	#[test]
	fn test_trust_level_ssr() {
		assert!(!TrustLevel::Untrusted.allows_ssr());
		assert!(TrustLevel::Verified.allows_ssr());
		assert!(TrustLevel::Trusted.allows_ssr());
	}
}
