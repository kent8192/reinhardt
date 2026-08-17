//! Prelude module for convenient imports
//!
//! Import this module to get access to the most commonly used types and traits.

pub use super::builder::{BuildError, GetError, MergeStrategy, MergedSettings, SettingsBuilder};
pub use super::cache::{CacheSettings, HasCacheSettings};
pub use super::composed::ResolvedSettings;
pub use super::cors::{CorsSettings, HasCorsSettings};
pub use super::email::{EmailSettings, HasEmailSettings};
pub use super::env::{Env, EnvError};
pub use super::env_loader::{EnvLoader, load_env, load_env_auto, load_env_optional};
pub use super::env_parser::{
	CacheUrl, DatabaseUrl, parse_bool, parse_cache_url, parse_database_url, parse_dict, parse_list,
};
pub use super::interpolation::InterpolationError;
pub use super::logging::{HasLoggingSettings, LoggingSettings};
pub use super::media::{HasMediaSettings, MediaSettings};
pub use super::migrations::{HasMigrationSettings, MigrationSettings};
pub use super::profile::Profile;
pub use super::schema::{
	FieldRef, HasSettingsSchema, MapRef, OptionalRef, ResolvedSettingsField, SecretFieldRef,
	SequenceRef, SettingsNode, SettingsNodeSchema, SettingsPathBuf, SettingsPathSegment,
	SettingsResolutionMetadata, SettingsRootSchema, SettingsRootSectionSchema, SettingsValueSchema,
	SettingsViolation, SettingsViolationKind, verify_settings_contract,
};
pub use super::session::{HasSessionSettings, SessionSettings};
pub use super::sources::{
	ConfigSource, DefaultSource, DotEnvSource, EnvSource, HighPriorityEnvSource,
	LowPriorityEnvSource, SourceError, TomlFileSource,
};
pub use super::static_files::{HasStaticSettings, StaticSettings};
pub use super::testing::{SettingsOverride, SettingsOverrideGuard};
pub use super::typed_deserializer::CoercionError;
pub use super::validation::{
	ChoiceValidator, PatternValidator, RangeValidator, RequiredValidator, SecurityValidator,
	SettingsValidator, ValidationError, ValidationResult, Validator,
};
pub use super::{DatabaseConfig, MiddlewareConfig};
// `TemplateConfig` is deprecated in favor of the `TemplateSettings` fragment;
// keep it in the prelude for the compatibility window.
#[allow(deprecated)]
pub use super::TemplateConfig;

// Dynamic settings (async feature)
#[cfg(feature = "async")]
pub use super::backends::{memory::MemoryBackend, *};

#[cfg(feature = "async")]
pub use super::dynamic::{DynamicBackend, DynamicError, DynamicResult, DynamicSettings};

#[cfg(feature = "async")]
pub use super::secrets::{
	SecretError, SecretManager, SecretMetadata, SecretProvider, SecretResult, SecretString,
	SecretValue, SecretVersion,
	providers::{env::EnvSecretProvider, memory::MemorySecretProvider},
};

#[cfg(feature = "vault")]
pub use super::secrets::providers::hashicorp::{VaultConfig, VaultSecretProvider};
