//! Monthly cost estimation engine for cloud deployments.
//!
//! Provides static pricing lookup tables per provider (AWS, GCP, fly.io)
//! and functions to estimate total monthly costs from a [`DeployConfig`].

use crate::config::{DeployConfig, InstanceSize, ProviderType};

/// Total monthly cost estimate for a deployment.
#[derive(Debug, Clone)]
pub struct CostEstimate {
	/// Cloud provider name.
	pub provider: String,
	/// Individual cost line items.
	pub items: Vec<CostLineItem>,
	/// Sum of all line items in USD per month.
	pub total_monthly_usd: f64,
}

/// A single line item in a cost estimate.
#[derive(Debug, Clone)]
pub struct CostLineItem {
	/// Resource identifier (e.g. "App Compute", "Database").
	pub resource: String,
	/// Human-readable description of the cost.
	pub description: String,
	/// Estimated monthly cost in USD.
	pub monthly_usd: f64,
}

/// Comparison between two cost estimates.
#[derive(Debug, Clone)]
pub struct CostDelta {
	/// Previous total monthly cost in USD.
	pub previous_total: f64,
	/// Current total monthly cost in USD.
	pub current_total: f64,
	/// Absolute difference (current - previous).
	pub delta: f64,
	/// Percentage change ((current - previous) / previous * 100).
	/// Returns 0.0 when previous_total is 0.
	pub delta_percent: f64,
}

// ---------------------------------------------------------------------------
// Pricing lookup tables
// ---------------------------------------------------------------------------

/// Estimated monthly USD for AWS ECS Fargate compute by instance size.
pub fn aws_compute_price(instance_size: &InstanceSize) -> f64 {
	match instance_size {
		InstanceSize::Micro => 15.0,
		InstanceSize::Small => 30.0,
		InstanceSize::Medium => 60.0,
		InstanceSize::Large => 120.0,
		InstanceSize::Xlarge => 240.0,
	}
}

/// Estimated monthly USD for AWS RDS by instance size.
pub fn aws_rds_price(instance_size: &InstanceSize) -> f64 {
	match instance_size {
		InstanceSize::Micro => 15.0,
		InstanceSize::Small => 25.0,
		InstanceSize::Medium => 50.0,
		InstanceSize::Large => 100.0,
		InstanceSize::Xlarge => 200.0,
	}
}

/// Estimated monthly USD for GCP Cloud Run compute by instance size.
pub fn gcp_compute_price(instance_size: &InstanceSize) -> f64 {
	match instance_size {
		InstanceSize::Micro => 10.0,
		InstanceSize::Small => 25.0,
		InstanceSize::Medium => 50.0,
		InstanceSize::Large => 100.0,
		InstanceSize::Xlarge => 200.0,
	}
}

/// Estimated monthly USD for GCP Cloud SQL by instance size.
pub fn gcp_cloud_sql_price(instance_size: &InstanceSize) -> f64 {
	match instance_size {
		InstanceSize::Micro => 10.0,
		InstanceSize::Small => 20.0,
		InstanceSize::Medium => 45.0,
		InstanceSize::Large => 90.0,
		InstanceSize::Xlarge => 180.0,
	}
}

/// Estimated monthly USD for Fly.io Machine compute by instance size.
pub fn fly_compute_price(instance_size: &InstanceSize) -> f64 {
	match instance_size {
		InstanceSize::Micro => 5.0,
		InstanceSize::Small => 15.0,
		InstanceSize::Medium => 30.0,
		InstanceSize::Large => 60.0,
		InstanceSize::Xlarge => 120.0,
	}
}

/// Estimated monthly USD for Fly.io Postgres by instance size.
pub fn fly_database_price(instance_size: &InstanceSize) -> f64 {
	match instance_size {
		InstanceSize::Micro => 10.0,
		InstanceSize::Small => 20.0,
		InstanceSize::Medium => 40.0,
		InstanceSize::Large => 80.0,
		InstanceSize::Xlarge => 160.0,
	}
}

/// Estimated monthly USD for a managed cache (Redis) by instance size.
pub fn cache_price(instance_size: &InstanceSize) -> f64 {
	match instance_size {
		InstanceSize::Micro => 10.0,
		InstanceSize::Small => 20.0,
		InstanceSize::Medium => 40.0,
		InstanceSize::Large => 80.0,
		InstanceSize::Xlarge => 160.0,
	}
}

// ---------------------------------------------------------------------------
// Instance size derivation from AppConfig cpu/memory
// ---------------------------------------------------------------------------

/// Derive an [`InstanceSize`] from CPU (mCPU) and memory (MB) values.
///
/// The mapping is based on common cloud provider instance tiers:
/// - Micro:  cpu <= 256,  memory <= 512
/// - Small:  cpu <= 512,  memory <= 1024
/// - Medium: cpu <= 1024, memory <= 2048
/// - Large:  cpu <= 2048, memory <= 4096
/// - Xlarge: anything larger
fn derive_instance_size(cpu: u32, memory: u32) -> InstanceSize {
	if cpu <= 256 && memory <= 512 {
		InstanceSize::Micro
	} else if cpu <= 512 && memory <= 1024 {
		InstanceSize::Small
	} else if cpu <= 1024 && memory <= 2048 {
		InstanceSize::Medium
	} else if cpu <= 2048 && memory <= 4096 {
		InstanceSize::Large
	} else {
		InstanceSize::Xlarge
	}
}

// ---------------------------------------------------------------------------
// Main estimation functions
// ---------------------------------------------------------------------------

/// Calculate the total monthly cost estimate for a deployment configuration.
///
/// Includes compute, database, and cache costs based on the configured
/// provider. Docker provider is treated as self-hosted with zero cost.
pub fn estimate_cost(config: &DeployConfig) -> CostEstimate {
	let provider_type = &config.provider.provider_type;
	let provider_name = match provider_type {
		ProviderType::Docker => "Docker",
		ProviderType::FlyIo => "Fly.io",
		ProviderType::Aws => "AWS",
		ProviderType::Gcp => "GCP",
	};

	let mut items = Vec::new();

	// Docker is self-hosted — no cloud costs.
	if *provider_type != ProviderType::Docker {
		// App compute cost
		let app_size = derive_instance_size(config.app.cpu, config.app.memory);
		let per_instance = match provider_type {
			ProviderType::Aws => aws_compute_price(&app_size),
			ProviderType::Gcp => gcp_compute_price(&app_size),
			ProviderType::FlyIo => fly_compute_price(&app_size),
			ProviderType::Docker => unreachable!(),
		};
		let instances = config.app.instances;
		let compute_total = per_instance * f64::from(instances);
		items.push(CostLineItem {
			resource: "App Compute".to_string(),
			description: format!("{instances}x {app_size:?} instance(s) on {provider_name}"),
			monthly_usd: compute_total,
		});

		// Database cost (if configured)
		if let Some(db) = &config.database {
			let db_price = match provider_type {
				ProviderType::Aws => aws_rds_price(&db.instance_size),
				ProviderType::Gcp => gcp_cloud_sql_price(&db.instance_size),
				ProviderType::FlyIo => fly_database_price(&db.instance_size),
				ProviderType::Docker => unreachable!(),
			};
			items.push(CostLineItem {
				resource: "Database".to_string(),
				description: format!("{:?} {:?} on {provider_name}", db.engine, db.instance_size,),
				monthly_usd: db_price,
			});
		}

		// Cache cost (if configured)
		if let Some(cache_cfg) = &config.cache {
			let cp = cache_price(&cache_cfg.instance_size);
			items.push(CostLineItem {
				resource: "Cache".to_string(),
				description: format!(
					"{} {:?} on {provider_name}",
					cache_cfg.engine, cache_cfg.instance_size,
				),
				monthly_usd: cp,
			});
		}
	}

	let total_monthly_usd: f64 = items.iter().map(|i| i.monthly_usd).sum();

	CostEstimate {
		provider: provider_name.to_string(),
		items,
		total_monthly_usd,
	}
}

/// Calculate the cost difference between a previous and current estimate.
pub fn calculate_delta(previous: &CostEstimate, current: &CostEstimate) -> CostDelta {
	let prev = previous.total_monthly_usd;
	let curr = current.total_monthly_usd;
	let delta = curr - prev;
	let delta_percent = if prev == 0.0 {
		0.0
	} else {
		(delta / prev) * 100.0
	};

	CostDelta {
		previous_total: prev,
		current_total: curr,
		delta,
		delta_percent,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::config::{AppConfig, CacheConfig, DatabaseConfig, DatabaseEngine, ProviderConfig};
	use rstest::rstest;

	// -----------------------------------------------------------------------
	// Helper: build a minimal DeployConfig for a given provider
	// -----------------------------------------------------------------------
	fn make_config(provider: ProviderType) -> DeployConfig {
		DeployConfig {
			provider: ProviderConfig {
				provider_type: provider,
			},
			app: AppConfig::default(),
			..Default::default()
		}
	}

	// =======================================================================
	// AWS pricing tests
	// =======================================================================

	#[rstest]
	#[case(InstanceSize::Micro, 15.0)]
	#[case(InstanceSize::Small, 30.0)]
	#[case(InstanceSize::Medium, 60.0)]
	#[case(InstanceSize::Large, 120.0)]
	#[case(InstanceSize::Xlarge, 240.0)]
	fn aws_compute_pricing(#[case] size: InstanceSize, #[case] expected: f64) {
		// Arrange & Act
		let price = aws_compute_price(&size);

		// Assert
		assert_eq!(price, expected);
	}

	// =======================================================================
	// GCP pricing tests
	// =======================================================================

	#[rstest]
	#[case(InstanceSize::Micro, 10.0)]
	#[case(InstanceSize::Small, 25.0)]
	#[case(InstanceSize::Medium, 50.0)]
	#[case(InstanceSize::Large, 100.0)]
	#[case(InstanceSize::Xlarge, 200.0)]
	fn gcp_compute_pricing(#[case] size: InstanceSize, #[case] expected: f64) {
		// Arrange & Act
		let price = gcp_compute_price(&size);

		// Assert
		assert_eq!(price, expected);
	}

	// =======================================================================
	// Fly.io pricing tests
	// =======================================================================

	#[rstest]
	#[case(InstanceSize::Micro, 5.0)]
	#[case(InstanceSize::Small, 15.0)]
	#[case(InstanceSize::Medium, 30.0)]
	#[case(InstanceSize::Large, 60.0)]
	#[case(InstanceSize::Xlarge, 120.0)]
	fn fly_compute_pricing(#[case] size: InstanceSize, #[case] expected: f64) {
		// Arrange & Act
		let price = fly_compute_price(&size);

		// Assert
		assert_eq!(price, expected);
	}

	// =======================================================================
	// Full estimate tests
	// =======================================================================

	#[rstest]
	fn estimate_aws_with_database_and_cache() {
		// Arrange
		let mut config = make_config(ProviderType::Aws);
		config.app.instances = 2;
		config.database = Some(DatabaseConfig {
			engine: DatabaseEngine::PostgreSql,
			version: None,
			instance_size: InstanceSize::Small,
			storage_gb: 20,
			backup_retention_days: 7,
			high_availability: false,
		});
		config.cache = Some(CacheConfig {
			engine: "redis".to_string(),
			version: None,
			instance_size: InstanceSize::Small,
		});

		// Act
		let estimate = estimate_cost(&config);

		// Assert
		// Default app: cpu=256, memory=512 => Micro => $15/instance, 2 instances => $30
		// Database: Small => $25
		// Cache: Small => $20
		assert_eq!(estimate.provider, "AWS");
		assert_eq!(estimate.items.len(), 3);
		assert_eq!(estimate.items[0].monthly_usd, 30.0); // 2 * $15
		assert_eq!(estimate.items[1].monthly_usd, 25.0);
		assert_eq!(estimate.items[2].monthly_usd, 20.0);
		assert_eq!(estimate.total_monthly_usd, 75.0);
	}

	#[rstest]
	fn estimate_gcp_with_database_only() {
		// Arrange
		let mut config = make_config(ProviderType::Gcp);
		config.database = Some(DatabaseConfig {
			engine: DatabaseEngine::PostgreSql,
			version: None,
			instance_size: InstanceSize::Medium,
			storage_gb: 50,
			backup_retention_days: 14,
			high_availability: true,
		});

		// Act
		let estimate = estimate_cost(&config);

		// Assert
		// App: cpu=256, memory=512 => Micro => $10, 1 instance => $10
		// Database: Medium => $45
		assert_eq!(estimate.provider, "GCP");
		assert_eq!(estimate.items.len(), 2);
		assert_eq!(estimate.items[0].monthly_usd, 10.0);
		assert_eq!(estimate.items[1].monthly_usd, 45.0);
		assert_eq!(estimate.total_monthly_usd, 55.0);
	}

	#[rstest]
	fn estimate_fly_minimal() {
		// Arrange
		let config = make_config(ProviderType::FlyIo);

		// Act
		let estimate = estimate_cost(&config);

		// Assert
		// App: cpu=256, memory=512 => Micro => $5, 1 instance => $5
		assert_eq!(estimate.provider, "Fly.io");
		assert_eq!(estimate.items.len(), 1);
		assert_eq!(estimate.items[0].monthly_usd, 5.0);
		assert_eq!(estimate.total_monthly_usd, 5.0);
	}

	#[rstest]
	fn estimate_docker_is_free() {
		// Arrange
		let mut config = make_config(ProviderType::Docker);
		config.database = Some(DatabaseConfig {
			engine: DatabaseEngine::PostgreSql,
			version: None,
			instance_size: InstanceSize::Large,
			storage_gb: 100,
			backup_retention_days: 30,
			high_availability: true,
		});
		config.cache = Some(CacheConfig {
			engine: "redis".to_string(),
			version: None,
			instance_size: InstanceSize::Large,
		});

		// Act
		let estimate = estimate_cost(&config);

		// Assert
		assert_eq!(estimate.provider, "Docker");
		assert!(estimate.items.is_empty());
		assert_eq!(estimate.total_monthly_usd, 0.0);
	}

	// =======================================================================
	// Cost delta tests
	// =======================================================================

	#[rstest]
	fn cost_delta_positive_scale_up() {
		// Arrange
		let previous = CostEstimate {
			provider: "AWS".to_string(),
			items: vec![],
			total_monthly_usd: 50.0,
		};
		let current = CostEstimate {
			provider: "AWS".to_string(),
			items: vec![],
			total_monthly_usd: 120.0,
		};

		// Act
		let delta = calculate_delta(&previous, &current);

		// Assert
		assert_eq!(delta.previous_total, 50.0);
		assert_eq!(delta.current_total, 120.0);
		assert_eq!(delta.delta, 70.0);
		assert_eq!(delta.delta_percent, 140.0);
	}

	#[rstest]
	fn cost_delta_negative_scale_down() {
		// Arrange
		let previous = CostEstimate {
			provider: "GCP".to_string(),
			items: vec![],
			total_monthly_usd: 200.0,
		};
		let current = CostEstimate {
			provider: "GCP".to_string(),
			items: vec![],
			total_monthly_usd: 80.0,
		};

		// Act
		let delta = calculate_delta(&previous, &current);

		// Assert
		assert_eq!(delta.previous_total, 200.0);
		assert_eq!(delta.current_total, 80.0);
		assert_eq!(delta.delta, -120.0);
		assert_eq!(delta.delta_percent, -60.0);
	}

	#[rstest]
	fn cost_delta_zero_no_change() {
		// Arrange
		let previous = CostEstimate {
			provider: "Fly.io".to_string(),
			items: vec![],
			total_monthly_usd: 100.0,
		};
		let current = CostEstimate {
			provider: "Fly.io".to_string(),
			items: vec![],
			total_monthly_usd: 100.0,
		};

		// Act
		let delta = calculate_delta(&previous, &current);

		// Assert
		assert_eq!(delta.delta, 0.0);
		assert_eq!(delta.delta_percent, 0.0);
	}

	#[rstest]
	fn cost_delta_from_zero_previous() {
		// Arrange
		let previous = CostEstimate {
			provider: "Docker".to_string(),
			items: vec![],
			total_monthly_usd: 0.0,
		};
		let current = CostEstimate {
			provider: "AWS".to_string(),
			items: vec![],
			total_monthly_usd: 75.0,
		};

		// Act
		let delta = calculate_delta(&previous, &current);

		// Assert
		assert_eq!(delta.delta, 75.0);
		// Percentage is 0 when previous is 0 (avoid division by zero).
		assert_eq!(delta.delta_percent, 0.0);
	}

	// =======================================================================
	// Derive instance size tests
	// =======================================================================

	#[rstest]
	#[case(256, 512, InstanceSize::Micro)]
	#[case(512, 1024, InstanceSize::Small)]
	#[case(1024, 2048, InstanceSize::Medium)]
	#[case(2048, 4096, InstanceSize::Large)]
	#[case(4096, 8192, InstanceSize::Xlarge)]
	fn derive_instance_size_mapping(
		#[case] cpu: u32,
		#[case] memory: u32,
		#[case] expected: InstanceSize,
	) {
		// Arrange & Act
		let size = derive_instance_size(cpu, memory);

		// Assert
		assert_eq!(size, expected);
	}
}
