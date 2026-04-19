/// Configuration for connecting to a Kafka cluster.
#[derive(Debug, Clone)]
pub struct KafkaConfig {
	pub brokers: Vec<String>,
	pub client_id: String,
}

impl KafkaConfig {
	pub fn new(brokers: impl IntoIterator<Item = impl Into<String>>) -> Self {
		Self {
			brokers: brokers.into_iter().map(Into::into).collect(),
			client_id: "reinhardt".to_owned(),
		}
	}

	pub fn with_client_id(mut self, id: impl Into<String>) -> Self {
		self.client_id = id.into();
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn default_client_id_is_reinhardt() {
		let config = KafkaConfig::new(["localhost:9092"]);
		assert_eq!(config.client_id, "reinhardt");
	}

	#[rstest]
	fn brokers_are_stored() {
		let config = KafkaConfig::new(["a:9092", "b:9092"]);
		assert_eq!(config.brokers, vec!["a:9092", "b:9092"]);
	}

	#[rstest]
	fn builder_overrides_client_id() {
		let config = KafkaConfig::new(["localhost:9092"]).with_client_id("my-app");
		assert_eq!(config.client_id, "my-app");
	}
}
