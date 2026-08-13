use jsonschema::draft202012;
use serde_json::{Value, json};

fn schema() -> Value {
	let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
		.ancestors()
		.nth(2)
		.expect("reinhardt-commands manifest must be nested under the repository crates directory");
	let path = root.join("website/static/schemas/application-contract/v0.json");
	serde_json::from_str(&std::fs::read_to_string(path).expect("read published v0 schema"))
		.expect("parse published v0 schema")
}

#[test]
fn minimal_contract_is_valid_and_closed() {
	let schema = schema();
	let validator = draft202012::new(&schema).expect("compile v0 schema");
	let document = json!({
		"$schema": "https://reinhardt-web.dev/schemas/application-contract/v0.json",
		"schema_version": 0,
		"models": [],
		"migrations": [],
		"routes": [],
		"settings": []
	});
	validator.validate(&document).expect("minimal v0 contract");
	let mut unexpected = document;
	unexpected["unexpected"] = json!(true);
	assert!(validator.validate(&unexpected).is_err());
}

#[test]
fn documentation_links_are_canonical() {
	let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
		.ancestors()
		.nth(2)
		.expect("repository root");
	for path in [
		"crates/reinhardt-commands/README.md",
		"crates/reinhardt-commands/src/lib.rs",
	] {
		let text = std::fs::read_to_string(root.join(path)).expect("read canonical documentation");
		assert_eq!(
			text.matches("https://reinhardt-web.dev/docs/application-contract/")
				.count(),
			1,
			"{path} should contain one canonical link"
		);
	}
	let flags = std::fs::read_to_string(root.join("website/content/docs/feature-flags.md"))
		.expect("read feature flag documentation");
	assert!(flags.contains("@/docs/application-contract.md"));
}

#[test]
fn exhaustive_v0_fixture_covers_all_producer_variants() {
	let schema = schema();
	let validator = draft202012::new(&schema).expect("compile v0 schema");
	let field_types = vec![
		json!({"kind": "big_integer"}),
		json!({"kind": "integer"}),
		json!({"kind": "small_integer"}),
		json!({"kind": "tiny_int"}),
		json!({"kind": "medium_int"}),
		json!({"kind": "char", "max_length": 10}),
		json!({"kind": "varchar", "max_length": 255}),
		json!({"kind": "text"}),
		json!({"kind": "tiny_text"}),
		json!({"kind": "medium_text"}),
		json!({"kind": "long_text"}),
		json!({"kind": "date"}),
		json!({"kind": "time"}),
		json!({"kind": "datetime"}),
		json!({"kind": "timestamp_tz"}),
		json!({"kind": "decimal", "precision": 12, "scale": 2}),
		json!({"kind": "float"}),
		json!({"kind": "double"}),
		json!({"kind": "real"}),
		json!({"kind": "boolean"}),
		json!({"kind": "binary"}),
		json!({"kind": "blob"}),
		json!({"kind": "tiny_blob"}),
		json!({"kind": "medium_blob"}),
		json!({"kind": "long_blob"}),
		json!({"kind": "bytea"}),
		json!({"kind": "json"}),
		json!({"kind": "json_binary"}),
		json!({"kind": "array", "element": {"kind": "uuid"}}),
		json!({"kind": "hstore"}),
		json!({"kind": "citext"}),
		json!({"kind": "int4_range"}),
		json!({"kind": "int8_range"}),
		json!({"kind": "num_range"}),
		json!({"kind": "date_range"}),
		json!({"kind": "ts_range"}),
		json!({"kind": "ts_tz_range"}),
		json!({"kind": "ts_vector"}),
		json!({"kind": "ts_query"}),
		json!({"kind": "vector", "dimensions": 3}),
		json!({"kind": "uuid"}),
		json!({"kind": "year"}),
		json!({"kind": "enum", "values": ["draft", "published"]}),
		json!({"kind": "enum", "values": [1, 2]}),
		json!({"kind": "set", "values": ["read", "write"]}),
		json!({"kind": "foreign_key", "to_table": "users", "to_field": "id", "on_delete": "cascade"}),
		json!({"kind": "one_to_one", "to": "auth.User", "on_delete": "cascade", "on_update": "no_action"}),
		json!({"kind": "many_to_many", "to": "tags.Tag", "through": null}),
		json!({"kind": "custom", "name": "ltree"}),
	];
	let fields: Vec<Value> = field_types
		.into_iter()
		.enumerate()
		.map(|(index, field_type)| {
			json!({
				"name": format!("field_{index}"),
				"type": field_type,
				"nullable": false,
				"primary_key": index == 0,
				"unique": false,
				"default": null,
				"generated": if index == 0 { json!({"expression": "id + 1", "storage": "stored"}) } else { Value::Null },
			})
		})
		.collect();
	let document = json!({
		"$schema": "https://reinhardt-web.dev/schemas/application-contract/v0.json",
		"schema_version": 0,
		"models": [{
			"app_label": "blog",
			"model_name": "Post",
			"table_name": "blog_posts",
			"fields": fields,
			"constraints": [{
				"name": "post_author_fk",
				"kind": "foreign_key",
				"fields": ["author_id"],
				"expression": null,
				"references": {"table": "users", "columns": ["id"], "on_delete": "cascade", "on_update": "no_action"}
			}],
			"indexes": [
				{"name": "post_hnsw", "fields": ["embedding"], "unique": false, "predicate": null, "method": {"kind": "hnsw", "m": 16, "ef_construction": 64}, "operator_class": "vector_l2_ops", "expressions": ["embedding"]},
				{"name": "post_ivf", "fields": ["embedding"], "unique": false, "predicate": null, "method": {"kind": "ivfflat", "lists": null}, "operator_class": null, "expressions": null}
			],
			"relationships": [{
				"field": "author_id",
				"kind": "foreign_key",
				"target": {"app_label": "accounts", "model_name": "User", "table_name": "users", "field_name": "id"},
				"related_name": "posts",
				"through_table": null,
				"on_delete": "cascade",
				"on_update": "no_action"
			}]
		}],
		"migrations": [{
			"app_label": "blog",
			"name": "0002_posts",
			"dependencies": [{"app_label": "blog", "name": "0001_initial"}],
			"replaces": [{"app_label": "blog", "name": "0001_initial"}],
			"applied": false
		}],
		"routes": [{"path": "/posts", "method": "GET", "name": null, "handler": "blog::views::list", "authentication": "protected", "guard": null}],
		"settings": [{"key_path": "core.secret_key", "rust_type": "String", "required": true, "has_default": false, "secret": true, "present": true}]
	});
	validator
		.validate(&document)
		.expect("exhaustive v0 contract");
}
