//! Integration tests for the reinhardt-deploy pipeline.
//!
//! These tests exercise multiple modules working together to validate
//! end-to-end deployment workflows: config parsing, feature detection,
//! HCL generation, pipeline tracking, reporting, cost estimation,
//! preview environments, and plan diffing.

use rstest::rstest;

use reinhardt_deploy::config::{
	AppConfig, CacheConfig, DatabaseConfig, DatabaseEngine, DeployConfig, InstanceSize,
	NetworkConfig, PreviewConfig, ProjectConfig, ProviderConfig, ProviderType,
};
use reinhardt_deploy::cost::estimate_cost;
use reinhardt_deploy::detection::feature_flags::analyze_feature_flags;
use reinhardt_deploy::init::{generate_deploy_toml, write_deploy_toml};
use reinhardt_deploy::pipeline::{PipelineResult, PipelineStage, StageResult};
use reinhardt_deploy::preview::create_preview_environment;
use reinhardt_deploy::providers::create_provider;
use reinhardt_deploy::report::{
	CheckReport, DeployReport, PlanSnapshot, PreflightReport, ResourceChange, TerraformReport,
	compare_plans, diff_exit_code, format_diff,
};

// ===========================================================================
// Test 1: Full config round-trip (file write -> load -> verify)
// ===========================================================================

#[rstest]
fn full_config_round_trip_via_file() {
	// Arrange
	let dir = tempfile::tempdir().unwrap();
	let deploy_toml = dir.path().join("deploy.toml");
	let toml_content = r#"
[project]
name = "integration-app"
region = "us-east-1"

[provider]
type = "docker"

[app]
port = 3000
health_check = "/api/health"
instances = 2
cpu = 512
memory = 1024

[database]
engine = "postgresql"
version = "16"
instance_size = "medium"
storage_gb = 50
backup_retention_days = 14
high_availability = true

[cache]
engine = "redis"
version = "7"
instance_size = "small"

[websockets]
enabled = true
path = "/ws/"

[network]
domain = "integration.example.com"
tls = true
force_https = true
"#;
	std::fs::write(&deploy_toml, toml_content).unwrap();

	// Act
	let config = DeployConfig::from_file(&deploy_toml).unwrap();

	// Assert
	assert_eq!(config.project.name, "integration-app");
	assert_eq!(config.project.region.as_deref(), Some("us-east-1"));
	assert_eq!(config.provider.provider_type, ProviderType::Docker);
	assert_eq!(config.app.port, 3000);
	assert_eq!(config.app.health_check, "/api/health");
	assert_eq!(config.app.instances, 2);
	assert_eq!(config.app.cpu, 512);
	assert_eq!(config.app.memory, 1024);

	let db = config.database.as_ref().unwrap();
	assert_eq!(db.engine, DatabaseEngine::PostgreSql);
	assert_eq!(db.version.as_deref(), Some("16"));
	assert_eq!(db.instance_size, InstanceSize::Medium);
	assert_eq!(db.storage_gb, 50);
	assert_eq!(db.backup_retention_days, 14);
	assert!(db.high_availability);

	let cache = config.cache.as_ref().unwrap();
	assert_eq!(cache.engine, "redis");
	assert_eq!(cache.version.as_deref(), Some("7"));
	assert_eq!(cache.instance_size, InstanceSize::Small);

	let ws = config.websockets.as_ref().unwrap();
	assert!(ws.enabled);
	assert_eq!(ws.path, "/ws/");

	assert_eq!(
		config.network.domain.as_deref(),
		Some("integration.example.com")
	);
	assert!(config.network.tls);
	assert!(config.network.force_https);
}

// ===========================================================================
// Test 2: Feature detection to deploy.toml generation
// ===========================================================================

#[rstest]
fn feature_detection_to_deploy_toml_generation() {
	// Arrange
	let features: Vec<String> = vec![
		"db-postgres".to_string(),
		"cache".to_string(),
		"websockets".to_string(),
	];

	// Act
	let detection = analyze_feature_flags(&features);
	let toml_content = generate_deploy_toml("detected-app", ProviderType::Docker, &detection);

	// Assert — verify detection results
	assert!(detection.database);
	assert_eq!(detection.database_engine, Some(DatabaseEngine::PostgreSql));
	assert!(detection.cache);
	assert!(detection.websockets);
	assert!(!detection.ambiguous);
	assert_eq!(detection.confidence, 1.0);

	// Assert — verify generated TOML sections
	assert!(toml_content.contains("[project]"));
	assert!(toml_content.contains("name = \"detected-app\""));
	assert!(toml_content.contains("[provider]"));
	assert!(toml_content.contains("type = \"docker\""));
	assert!(toml_content.contains("[database]"));
	assert!(toml_content.contains("engine = \"postgresql\""));
	assert!(toml_content.contains("[cache]"));
	assert!(toml_content.contains("engine = \"redis\""));
	assert!(toml_content.contains("[websockets]"));
	assert!(toml_content.contains("enabled = true"));
}

// ===========================================================================
// Test 3: Provider creation and HCL generation
// ===========================================================================

#[rstest]
fn provider_creation_and_hcl_generation_docker() {
	// Arrange
	let config = DeployConfig {
		project: ProjectConfig {
			name: "hcl-test-app".to_string(),
			region: Some("us-east-1".to_string()),
		},
		provider: ProviderConfig {
			provider_type: ProviderType::Docker,
		},
		app: AppConfig {
			port: 8000,
			health_check: "/health/".to_string(),
			instances: 1,
			cpu: 256,
			memory: 512,
			env_file: None,
		},
		database: Some(DatabaseConfig {
			engine: DatabaseEngine::PostgreSql,
			version: Some("16".to_string()),
			instance_size: InstanceSize::Small,
			storage_gb: 20,
			backup_retention_days: 7,
			high_availability: false,
		}),
		cache: Some(CacheConfig {
			engine: "redis".to_string(),
			version: Some("7".to_string()),
			instance_size: InstanceSize::Small,
		}),
		..Default::default()
	};

	// Act
	let provider = create_provider(ProviderType::Docker);
	let hcl_files = provider.generate_hcl(&config).unwrap();

	// Assert
	assert_eq!(provider.name(), "docker");
	assert!(!hcl_files.is_empty());

	// Verify that at least one generated file contains expected Terraform blocks
	let all_hcl: String = hcl_files.values().cloned().collect::<Vec<_>>().join("\n");
	assert!(
		all_hcl.contains("terraform") || all_hcl.contains("resource") || all_hcl.contains("docker"),
		"HCL output should contain terraform, resource, or docker references"
	);
}

// ===========================================================================
// Test 4: Pipeline result tracking
// ===========================================================================

#[rstest]
fn pipeline_result_tracking_with_mixed_stages() {
	// Arrange
	let mut pipeline = PipelineResult::new(true);

	// Act — add successful stages followed by a failure
	pipeline.add_stage(StageResult {
		stage: PipelineStage::ConfigParse,
		success: true,
		message: "Configuration parsed successfully".to_string(),
		duration_ms: 15,
	});
	pipeline.add_stage(StageResult {
		stage: PipelineStage::FeatureDetection,
		success: true,
		message: "Features detected".to_string(),
		duration_ms: 120,
	});
	pipeline.add_stage(StageResult {
		stage: PipelineStage::PreflightChecks,
		success: true,
		message: "All checks passed".to_string(),
		duration_ms: 250,
	});
	pipeline.add_stage(StageResult {
		stage: PipelineStage::Build,
		success: false,
		message: "Docker build failed: image not found".to_string(),
		duration_ms: 5000,
	});
	pipeline.add_stage(StageResult {
		stage: PipelineStage::TerraformGenerate,
		success: true,
		message: "HCL files generated".to_string(),
		duration_ms: 30,
	});

	// Assert
	assert!(pipeline.dry_run);
	assert!(!pipeline.success);
	assert_eq!(pipeline.stages.len(), 5);

	let failed = pipeline.failed_stage().unwrap();
	assert_eq!(failed.stage, PipelineStage::Build);
	assert_eq!(failed.message, "Docker build failed: image not found");
	assert_eq!(failed.duration_ms, 5000);
}

// ===========================================================================
// Test 5: Report generation end-to-end
// ===========================================================================

#[rstest]
fn report_generation_all_formats() {
	// Arrange
	let report = DeployReport {
		version: "0.1.0".to_string(),
		timestamp: "2026-02-15T10:00:00Z".to_string(),
		commit: "a1b2c3d".to_string(),
		environment: "staging".to_string(),
		provider: "docker".to_string(),
		dry_run: true,
		preflight: PreflightReport {
			passed: true,
			checks: vec![
				CheckReport {
					name: "terraform".to_string(),
					passed: true,
					message: "v1.11.2 installed".to_string(),
				},
				CheckReport {
					name: "docker".to_string(),
					passed: true,
					message: "Docker daemon running".to_string(),
				},
			],
		},
		terraform: TerraformReport {
			creates: 5,
			updates: 1,
			destroys: 0,
			unchanged: 12,
			drift: false,
			changes: vec![ResourceChange {
				action: "create".to_string(),
				resource_type: "docker_container".to_string(),
				resource_name: "app".to_string(),
			}],
		},
		exit_code: 2,
	};

	// Act
	let human = report.format_human();
	let json = report.format_json().unwrap();
	let markdown = report.format_markdown();

	// Assert — human format
	assert!(human.contains("DRY-RUN REPORT"));
	assert!(human.contains("staging"));
	assert!(human.contains("docker"));
	assert!(human.contains("a1b2c3d"));
	assert!(human.contains("Pre-flight Checks: PASSED"));
	assert!(human.contains("[OK] terraform"));
	assert!(human.contains("[OK] docker"));
	assert!(human.contains("Create:    5"));
	assert!(human.contains("Update:    1"));
	assert!(human.contains("Destroy:   0"));

	// Assert — JSON format and round-trip
	assert!(json.contains("\"version\": \"0.1.0\""));
	assert!(json.contains("\"dry_run\": true"));
	assert!(json.contains("\"environment\": \"staging\""));
	let deserialized: DeployReport = serde_json::from_str(&json).unwrap();
	assert_eq!(deserialized.version, "0.1.0");
	assert_eq!(deserialized.commit, "a1b2c3d");
	assert_eq!(deserialized.environment, "staging");
	assert_eq!(deserialized.terraform.creates, 5);
	assert_eq!(deserialized.terraform.updates, 1);
	assert!(deserialized.dry_run);
	assert_eq!(deserialized.exit_code, 2);

	// Assert — markdown format
	assert!(markdown.contains("## Dry-Run Report"));
	assert!(markdown.contains("| Environment | staging |"));
	assert!(markdown.contains("| Provider | docker |"));
	assert!(markdown.contains("| `a1b2c3d` |"));
	assert!(markdown.contains("| Create | 5 |"));
	assert!(markdown.contains("| Update | 1 |"));
}

// ===========================================================================
// Test 6: Cost estimation with config
// ===========================================================================

#[rstest]
fn cost_estimation_aws_with_database_and_cache() {
	// Arrange
	let config = DeployConfig {
		provider: ProviderConfig {
			provider_type: ProviderType::Aws,
		},
		app: AppConfig {
			port: 8000,
			health_check: "/health/".to_string(),
			instances: 2,
			cpu: 256,
			memory: 512,
			env_file: None,
		},
		database: Some(DatabaseConfig {
			engine: DatabaseEngine::PostgreSql,
			version: Some("16".to_string()),
			instance_size: InstanceSize::Small,
			storage_gb: 20,
			backup_retention_days: 7,
			high_availability: false,
		}),
		cache: Some(CacheConfig {
			engine: "redis".to_string(),
			version: Some("7".to_string()),
			instance_size: InstanceSize::Small,
		}),
		..Default::default()
	};

	// Act
	let estimate = estimate_cost(&config);

	// Assert
	assert_eq!(estimate.provider, "AWS");
	assert!(estimate.total_monthly_usd > 0.0);
	assert_eq!(estimate.items.len(), 3);

	// Verify specific resources exist in line items
	let resource_names: Vec<&str> = estimate.items.iter().map(|i| i.resource.as_str()).collect();
	assert!(resource_names.contains(&"App Compute"));
	assert!(resource_names.contains(&"Database"));
	assert!(resource_names.contains(&"Cache"));

	// App Compute: Micro (cpu=256, memory=512) => $15/instance * 2 = $30
	assert_eq!(estimate.items[0].monthly_usd, 30.0);
	// Database: Small => $25
	assert_eq!(estimate.items[1].monthly_usd, 25.0);
	// Cache: Small => $20
	assert_eq!(estimate.items[2].monthly_usd, 20.0);
	// Total: $75
	assert_eq!(estimate.total_monthly_usd, 75.0);
}

// ===========================================================================
// Test 7: Preview environment creation
// ===========================================================================

#[rstest]
fn preview_environment_creation_with_domain() {
	// Arrange
	let config = DeployConfig {
		network: NetworkConfig {
			domain: Some("myapp.example.com".to_string()),
			tls: true,
			force_https: true,
			websocket: false,
			grpc: false,
		},
		preview: Some(PreviewConfig {
			enabled: true,
			auto_deploy: true,
			branch_subdomains: true,
			ttl_hours: 48,
			shared_database: false,
			seed_data: false,
		}),
		..Default::default()
	};

	// Act
	let env = create_preview_environment(42, &config).unwrap();

	// Assert
	assert_eq!(env.pr_number, 42);
	assert_eq!(env.workspace_name, "preview-pr-42");
	assert_eq!(env.subdomain, "pr-42.preview.myapp.example.com");
	assert_eq!(env.ttl_hours, 48);

	// Verify scaled-down resources
	assert_eq!(env.scaled_config.instances, 1);
	assert_eq!(env.scaled_config.cpu, 128);
	assert_eq!(env.scaled_config.memory, 256);
	assert_eq!(env.scaled_config.instance_size, InstanceSize::Micro);
}

// ===========================================================================
// Test 8: Plan diff comparison
// ===========================================================================

#[rstest]
fn plan_diff_identical_then_modified() {
	// Arrange — two identical snapshots
	let snapshot_a = PlanSnapshot {
		plan_hash: "hash_aaa".to_string(),
		config_hash: "cfg_111".to_string(),
		state_hash: "state_xyz".to_string(),
		timestamp: "2026-02-15T10:00:00Z".to_string(),
		terraform: TerraformReport {
			creates: 3,
			updates: 1,
			destroys: 0,
			unchanged: 10,
			drift: false,
			changes: vec![],
		},
	};
	let snapshot_b = snapshot_a.clone();

	// Act — compare identical
	let diff_identical = compare_plans(&snapshot_a, &snapshot_b);

	// Assert — identical
	assert!(diff_identical.identical);
	assert!(diff_identical.differences.is_empty());
	assert_eq!(diff_exit_code(&diff_identical), 0);
	assert_eq!(format_diff(&diff_identical), "Plans are IDENTICAL");

	// Arrange — modify the second snapshot
	let mut snapshot_c = snapshot_a.clone();
	snapshot_c.plan_hash = "hash_bbb".to_string();
	snapshot_c.terraform.creates = 7;
	snapshot_c.terraform.destroys = 2;

	// Act — compare different
	let diff_changed = compare_plans(&snapshot_a, &snapshot_c);

	// Assert — differs
	assert!(!diff_changed.identical);
	assert_eq!(diff_changed.differences.len(), 3);
	assert_eq!(diff_exit_code(&diff_changed), 1);

	let diff_output = format_diff(&diff_changed);
	assert!(diff_output.contains("Plans DIFFER:"));
	assert!(diff_output.contains("plan_hash"));
	assert!(diff_output.contains("terraform.creates"));
	assert!(diff_output.contains("terraform.destroys"));

	// Verify specific difference values
	let fields: Vec<&str> = diff_changed
		.differences
		.iter()
		.map(|d| d.field.as_str())
		.collect();
	assert!(fields.contains(&"plan_hash"));
	assert!(fields.contains(&"terraform.creates"));
	assert!(fields.contains(&"terraform.destroys"));
}

// ===========================================================================
// Test 9: Deploy.toml write and reload
// ===========================================================================

#[rstest]
fn deploy_toml_write_and_reload() {
	// Arrange
	let dir = tempfile::tempdir().unwrap();
	let detection = analyze_feature_flags(&["db-postgres".to_string(), "cache".to_string()]);
	let content = generate_deploy_toml("reload-app", ProviderType::Docker, &detection);

	// Act — write and reload
	write_deploy_toml(dir.path(), &content).unwrap();
	let reloaded = DeployConfig::load_or_default(dir.path()).unwrap();

	// Assert
	assert_eq!(reloaded.project.name, "reload-app");
	assert_eq!(reloaded.provider.provider_type, ProviderType::Docker);
	assert!(reloaded.database.is_some());
	let db = reloaded.database.as_ref().unwrap();
	assert_eq!(db.engine, DatabaseEngine::PostgreSql);
	assert!(reloaded.cache.is_some());
	assert_eq!(reloaded.cache.as_ref().unwrap().engine, "redis");
}

// ===========================================================================
// Test 10: Init detection to HCL pipeline (end-to-end)
// ===========================================================================

#[rstest]
fn end_to_end_detection_to_hcl_pipeline() {
	// Arrange — analyze feature flags
	let features: Vec<String> = vec![
		"db-postgres".to_string(),
		"cache".to_string(),
		"websockets".to_string(),
	];
	let detection = analyze_feature_flags(&features);

	// Act — generate deploy.toml from detection
	let toml_content = generate_deploy_toml("e2e-app", ProviderType::Docker, &detection);

	// Write to temp dir and reload config
	let dir = tempfile::tempdir().unwrap();
	write_deploy_toml(dir.path(), &toml_content).unwrap();
	let config = DeployConfig::from_file(&dir.path().join("deploy.toml")).unwrap();

	// Create provider and generate HCL
	let provider = create_provider(config.provider.provider_type.clone());
	let hcl_files = provider.generate_hcl(&config).unwrap();

	// Assert — verify end-to-end pipeline worked
	assert_eq!(config.project.name, "e2e-app");
	assert_eq!(config.provider.provider_type, ProviderType::Docker);
	assert!(config.database.is_some());
	assert!(config.cache.is_some());
	assert!(config.websockets.is_some());
	assert_eq!(provider.name(), "docker");
	assert!(!hcl_files.is_empty());

	// Verify HCL file names are non-empty strings
	for (filename, content) in &hcl_files {
		assert!(!filename.is_empty(), "HCL filename should not be empty");
		assert!(!content.is_empty(), "HCL content should not be empty");
	}

	// Verify pipeline result can track this flow
	let mut pipeline = PipelineResult::new(false);
	pipeline.add_stage(StageResult {
		stage: PipelineStage::ConfigParse,
		success: true,
		message: "Loaded from generated deploy.toml".to_string(),
		duration_ms: 5,
	});
	pipeline.add_stage(StageResult {
		stage: PipelineStage::FeatureDetection,
		success: true,
		message: format!(
			"Detected: db={}, cache={}, ws={}",
			detection.database, detection.cache, detection.websockets
		),
		duration_ms: 10,
	});
	pipeline.add_stage(StageResult {
		stage: PipelineStage::TerraformGenerate,
		success: true,
		message: format!("Generated {} HCL files", hcl_files.len()),
		duration_ms: 25,
	});

	assert!(pipeline.success);
	assert!(pipeline.failed_stage().is_none());
	assert_eq!(pipeline.stages.len(), 3);
}
