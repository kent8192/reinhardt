//! Settings and configuration type re-exports.

pub use reinhardt_conf::SecuritySettings;
pub use reinhardt_conf::settings::ResolvedSettings;
pub use reinhardt_conf::settings::builder::SettingsBuilder;
pub use reinhardt_conf::settings::core_settings::{CoreSettings, HasCoreSettings};
pub use reinhardt_conf::settings::fragment::{HasSettings, SettingsFragment};
pub use reinhardt_conf::settings::profile::Profile;
pub use reinhardt_conf::settings::sources::{
	DefaultSource, EnvSource, LowPriorityEnvSource, TomlFileSource,
};
// This block re-exports the deprecated `TemplateConfig` during the 0.2
// compatibility window; the settings-first replacement is `TemplateSettings`.
#[allow(deprecated)]
pub use reinhardt_conf::settings::{DatabaseConfig, MiddlewareConfig, TemplateConfig};
pub use reinhardt_conf::{
	CacheSettings, CorsSettings, EmailSettings, LoggingSettings, MediaSettings, MigrationSettings,
	SessionSettings, StaticSettings,
};
