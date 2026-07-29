#![cfg(feature = "pages")]

use std::collections::BTreeMap;

use reinhardt_commands::template_manifest::collect_template_source;
use reinhardt_commands::{
	CompiledBaseline, RebuildReason, SourceBaseline, StaticOverlayStore, TemplateClassification,
	classify_source_change,
};
use reinhardt_pages::hmr::{CompiledBuildId, SourceId, StaticTemplateNode};
use tempfile::TempDir;

fn fixture_source(static_text: &str) -> String {
	format!(
		r#"
fn render() {{
    let page = page!(|| {{
        div {{
            class: "shell",
            "{static_text}"
        }}
    }});
    page
}}
"#
	)
}

fn write_fixture(source: &str) -> (TempDir, std::path::PathBuf) {
	let root = tempfile::tempdir().expect("temporary project should be created");
	let path = root.path().join("src/page.rs");
	std::fs::create_dir_all(path.parent().expect("fixture has a parent"))
		.expect("fixture directory should be created");
	std::fs::write(&path, source).expect("fixture source should be written");
	(root, path)
}

fn baseline(root: &TempDir, path: &std::path::Path, source: &str) -> CompiledBaseline {
	let source_id = SourceId("src/page.rs".to_owned());
	let parsed = collect_template_source(&source_id, source).expect("fixture should lower");
	let callsites = parsed
		.templates
		.iter()
		.map(|template| template.key.clone())
		.collect::<Vec<_>>();
	let descriptors = parsed
		.templates
		.into_iter()
		.map(|template| (template.key, template.descriptor))
		.collect::<BTreeMap<_, _>>();
	let mut baseline = CompiledBaseline::new(CompiledBuildId([7; 32]), [9; 32]);
	baseline.sources.insert(
		source_id,
		SourceBaseline {
			source_text: source.to_owned(),
			callsites,
			descriptors,
		},
	);
	assert!(path.starts_with(root.path()));
	baseline
}

#[test]
fn static_template_edit_produces_patch_with_compiled_identity() {
	let original = fixture_source("Hello");
	let changed = fixture_source("Welcome");
	let (root, path) = write_fixture(&changed);
	let baseline = baseline(&root, &path, &original);

	let classification = classify_source_change(
		root.path(),
		std::slice::from_ref(&path),
		&baseline,
		&StaticOverlayStore::new(),
	);
	let TemplateClassification::Patchable(patches) = classification else {
		panic!("literal-only edit should be patchable");
	};
	assert_eq!(patches.build_id, CompiledBuildId([7; 32]));
	assert_eq!(patches.manifest_digest, [9; 32]);
	assert_eq!(patches.patches.len(), 1);
	assert_eq!(
		patches.patches[0].key,
		baseline.sources[&SourceId("src/page.rs".to_owned())].callsites[0]
	);
	assert!(patches.patches[0].placements.is_empty());
}

#[test]
fn overlay_is_compared_against_candidate_static_tree() {
	let original = fixture_source("Hello");
	let changed = fixture_source("Welcome");
	let (root, path) = write_fixture(&changed);
	let baseline = baseline(&root, &path, &original);
	let mut overlays = StaticOverlayStore::new();

	let first = classify_source_change(
		root.path(),
		std::slice::from_ref(&path),
		&baseline,
		&overlays,
	);
	let TemplateClassification::Patchable(first_patch_set) = first else {
		panic!("first literal edit should be patchable");
	};
	overlays.install(first_patch_set.generation, &first_patch_set.patches);

	std::fs::write(&path, &original).expect("source should be reverted");
	let second = classify_source_change(
		root.path(),
		std::slice::from_ref(&path),
		&baseline,
		&overlays,
	);
	let TemplateClassification::Patchable(second_patch_set) = second else {
		panic!("reverting an overlay should produce a patch");
	};
	assert_eq!(second_patch_set.patches.len(), 1);
	assert!(matches!(
		second_patch_set.patches[0].static_tree,
		StaticTemplateNode::Element { ref children, .. }
			if matches!(children.as_slice(), [StaticTemplateNode::Text(text)] if text == "Hello")
	));
}

#[test]
fn dynamic_abi_change_requires_rebuild() {
	let original = r#"
fn render() {
    let page = page!(|count: i32| { div { { count } } });
    page
}
"#;
	let changed = r#"
fn render() {
    let page = page!(|count: i32| { div { { count + 1 } } });
    page
}
"#;
	let (root, path) = write_fixture(changed);
	let baseline = baseline(&root, &path, original);

	assert_eq!(
		classify_source_change(
			root.path(),
			std::slice::from_ref(&path),
			&baseline,
			&StaticOverlayStore::new(),
		),
		TemplateClassification::RebuildRequired(RebuildReason::DynamicAbiChanged)
	);
}

#[test]
fn typed_custom_payload_change_with_static_edit_requires_dynamic_abi_rebuild() {
	let original = r#"
fn render() {
    let page = page!(|| {
        button {
            @custom::<u64>("item-selected"): |_| {},
            "Before"
        }
    });
    page
}
"#;
	let changed = r#"
fn render() {
    let page = page!(|| {
        button {
            @custom::<String>("item-selected"): |_| {},
            "After"
        }
    });
    page
}
"#;
	let (root, path) = write_fixture(changed);
	let baseline = baseline(&root, &path, original);

	assert_eq!(
		classify_source_change(
			root.path(),
			std::slice::from_ref(&path),
			&baseline,
			&StaticOverlayStore::new(),
		),
		TemplateClassification::RebuildRequired(RebuildReason::DynamicAbiChanged)
	);
}

#[test]
fn static_sibling_of_direct_dynamic_attribute_is_patchable() {
	let original = r#"
fn render() {
    let page = page!(|class_name: String| {
        div { class: class_name, "Live" }
        span { "Before" }
    });
    page
}
"#;
	let changed = original.replace("Before", "After");
	let (root, path) = write_fixture(&changed);
	let baseline = baseline(&root, &path, original);

	let TemplateClassification::Patchable(patches) = classify_source_change(
		root.path(),
		std::slice::from_ref(&path),
		&baseline,
		&StaticOverlayStore::new(),
	) else {
		panic!("a static sibling should retain the bound element range");
	};
	assert_eq!(patches.patches.len(), 1);
	assert_eq!(patches.patches[0].placements.len(), 1);
}

#[test]
fn static_content_inside_direct_dynamic_attribute_requires_safe_fallback() {
	let original = r#"
fn render() {
    let page = page!(|class_name: String| {
        div { class: class_name, "Before" }
    });
    page
}
"#;
	let changed = original.replace("Before", "After");
	let (root, path) = write_fixture(&changed);
	let baseline = baseline(&root, &path, original);

	assert_eq!(
		classify_source_change(
			root.path(),
			std::slice::from_ref(&path),
			&baseline,
			&StaticOverlayStore::new(),
		),
		TemplateClassification::RebuildRequired(RebuildReason::StaticContentOutsidePatchTree)
	);
}

#[test]
fn nested_template_static_edit_requires_safe_fallback() {
	let original = r#"
fn render() {
    let page = page!(|show: bool| {
        if show { div { "Before" } } else { div { "Else" } }
    });
    page
}
"#;
	let changed = original.replace("Before", "After");
	let (root, path) = write_fixture(&changed);
	let baseline = baseline(&root, &path, original);

	assert_eq!(
		classify_source_change(
			root.path(),
			std::slice::from_ref(&path),
			&baseline,
			&StaticOverlayStore::new(),
		),
		TemplateClassification::RebuildRequired(RebuildReason::NestedTemplateChanged)
	);
}

#[test]
fn code_outside_page_body_requires_shared_rebuild() {
	let original = format!("const VERSION: u8 = 1;\n{}", fixture_source("Hello"));
	let changed = format!("const VERSION: u8 = 2;\n{}", fixture_source("Hello"));
	let (root, path) = write_fixture(&changed);
	let baseline = baseline(&root, &path, &original);

	assert_eq!(
		classify_source_change(
			root.path(),
			std::slice::from_ref(&path),
			&baseline,
			&StaticOverlayStore::new(),
		),
		TemplateClassification::RebuildRequired(RebuildReason::SharedOrSsrSourceChanged)
	);
}

#[test]
fn malformed_page_body_returns_relative_diagnostic() {
	let original = fixture_source("Hello");
	let changed = "fn render() { let page = page!(|| { div { ); page }";
	let (root, path) = write_fixture(changed);
	let baseline = baseline(&root, &path, &original);

	let TemplateClassification::InvalidTemplate(diagnostic) = classify_source_change(
		root.path(),
		std::slice::from_ref(&path),
		&baseline,
		&StaticOverlayStore::new(),
	) else {
		panic!("malformed page body should produce a diagnostic");
	};
	assert_eq!(diagnostic.source_id, SourceId("src/page.rs".to_owned()));
	assert!(!diagnostic.message.is_empty());
}
