//! Integration tests for the migration dependency graph
//!
//! Tests cover `MigrationKey`, `MigrationNode`, and `MigrationGraph` from
//! `reinhardt_db::migrations::graph`.

use rstest::rstest;

use reinhardt_db::migrations::graph::{MigrationGraph, MigrationKey, MigrationNode};

// ---------------------------------------------------------------------------
// MigrationKey tests
// ---------------------------------------------------------------------------

#[rstest]
fn migration_key_new_stores_fields() {
	// Arrange
	let app = "auth";
	let name = "0001_initial";

	// Act
	let key = MigrationKey::new(app, name);

	// Assert
	assert_eq!(key.app_label, "auth");
	assert_eq!(key.name, "0001_initial");
}

#[rstest]
fn migration_key_id_returns_dot_separated() {
	// Arrange
	let key = MigrationKey::new("users", "0002_add_email");

	// Act
	let id = key.id();

	// Assert
	assert_eq!(id, "users.0002_add_email");
}

#[rstest]
fn migration_key_display_matches_id() {
	// Arrange
	let key = MigrationKey::new("blog", "0003_alter_title");

	// Act
	let display = format!("{}", key);

	// Assert
	assert_eq!(display, key.id());
}

#[rstest]
fn migration_key_clone_produces_equal_key() {
	// Arrange
	let key = MigrationKey::new("app", "migration");

	// Act
	let cloned = key.clone();

	// Assert
	assert_eq!(key, cloned);
}

#[rstest]
fn migration_key_equality() {
	// Arrange
	let key_a = MigrationKey::new("app", "name");
	let key_b = MigrationKey::new("app", "name");
	let key_c = MigrationKey::new("app", "other");

	// Act

	// Assert
	assert_eq!(key_a, key_b);
	assert_ne!(key_a, key_c);
}

#[rstest]
fn migration_key_hash_consistency() {
	// Arrange
	use std::collections::HashSet;
	let key_a = MigrationKey::new("app", "name");
	let key_b = MigrationKey::new("app", "name");

	// Act
	let mut set = HashSet::new();
	set.insert(key_a);
	set.insert(key_b);

	// Assert - equal keys should collapse into one entry
	assert_eq!(set.len(), 1);
}

// ---------------------------------------------------------------------------
// MigrationNode tests
// ---------------------------------------------------------------------------

#[rstest]
fn migration_node_new_creates_node_without_replaces() {
	// Arrange
	let key = MigrationKey::new("auth", "0001_initial");
	let dep = MigrationKey::new("contenttypes", "0001_initial");

	// Act
	let node = MigrationNode::new(key.clone(), vec![dep.clone()]);

	// Assert
	assert_eq!(node.key, key);
	assert_eq!(node.dependencies.len(), 1);
	assert_eq!(node.dependencies[0], dep);
	assert!(node.replaces.is_empty());
}

#[rstest]
fn migration_node_with_replaces_stores_replaces() {
	// Arrange
	let key = MigrationKey::new("auth", "0001_squashed_0003");
	let old1 = MigrationKey::new("auth", "0001_initial");
	let old2 = MigrationKey::new("auth", "0002_add_field");

	// Act
	let node = MigrationNode::with_replaces(key.clone(), vec![], vec![old1.clone(), old2.clone()]);

	// Assert
	assert_eq!(node.key, key);
	assert!(node.dependencies.is_empty());
	assert_eq!(node.replaces.len(), 2);
	assert_eq!(node.replaces[0], old1);
	assert_eq!(node.replaces[1], old2);
}

// ---------------------------------------------------------------------------
// MigrationGraph - basic operations
// ---------------------------------------------------------------------------

#[rstest]
fn graph_new_is_empty() {
	// Arrange

	// Act
	let graph = MigrationGraph::new();

	// Assert
	assert!(graph.is_empty());
	assert_eq!(graph.len(), 0);
}

#[rstest]
fn graph_add_migration_increments_len() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let key = MigrationKey::new("auth", "0001_initial");

	// Act
	graph.add_migration(key.clone(), vec![]);

	// Assert
	assert_eq!(graph.len(), 1);
	assert!(!graph.is_empty());
}

#[rstest]
fn graph_has_migration_returns_correct_bool() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let key = MigrationKey::new("auth", "0001_initial");
	let missing = MigrationKey::new("auth", "9999_missing");

	// Act
	graph.add_migration(key.clone(), vec![]);

	// Assert
	assert!(graph.has_migration(&key));
	assert!(!graph.has_migration(&missing));
}

#[rstest]
fn graph_get_node_returns_correct_node() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let key = MigrationKey::new("auth", "0001_initial");
	graph.add_migration(key.clone(), vec![]);

	// Act
	let node = graph.get_node(&key);

	// Assert
	assert!(node.is_some());
	assert_eq!(node.unwrap().key, key);
}

#[rstest]
fn graph_get_node_returns_none_for_missing() {
	// Arrange
	let graph = MigrationGraph::new();
	let key = MigrationKey::new("auth", "0001_initial");

	// Act
	let node = graph.get_node(&key);

	// Assert
	assert!(node.is_none());
}

#[rstest]
fn graph_get_dependencies_returns_deps() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let key1 = MigrationKey::new("auth", "0001_initial");
	let key2 = MigrationKey::new("auth", "0002_add_field");
	graph.add_migration(key1.clone(), vec![]);
	graph.add_migration(key2.clone(), vec![key1.clone()]);

	// Act
	let deps = graph.get_dependencies(&key2).unwrap();

	// Assert
	assert_eq!(deps.len(), 1);
	assert_eq!(deps[0], key1);
}

#[rstest]
fn graph_get_dependents_returns_dependents() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let key1 = MigrationKey::new("auth", "0001_initial");
	let key2 = MigrationKey::new("auth", "0002_add_field");
	graph.add_migration(key1.clone(), vec![]);
	graph.add_migration(key2.clone(), vec![key1.clone()]);

	// Act
	let dependents = graph.get_dependents(&key1);

	// Assert
	assert_eq!(dependents.len(), 1);
	assert_eq!(dependents[0], &key2);
}

#[rstest]
fn graph_all_migrations_returns_all_keys() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let key1 = MigrationKey::new("auth", "0001_initial");
	let key2 = MigrationKey::new("users", "0001_initial");
	graph.add_migration(key1.clone(), vec![]);
	graph.add_migration(key2.clone(), vec![]);

	// Act
	let all = graph.all_migrations();

	// Assert
	assert_eq!(all.len(), 2);
	assert!(all.contains(&&key1));
	assert!(all.contains(&&key2));
}

#[rstest]
fn graph_remove_migration_removes_node() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let key = MigrationKey::new("auth", "0001_initial");
	graph.add_migration(key.clone(), vec![]);

	// Act
	graph.remove_migration(&key);

	// Assert
	assert!(!graph.has_migration(&key));
	assert_eq!(graph.len(), 0);
}

#[rstest]
fn graph_clear_removes_all_nodes() {
	// Arrange
	let mut graph = MigrationGraph::new();
	graph.add_migration(MigrationKey::new("a", "1"), vec![]);
	graph.add_migration(MigrationKey::new("b", "1"), vec![]);

	// Act
	graph.clear();

	// Assert
	assert!(graph.is_empty());
	assert_eq!(graph.len(), 0);
}

// ---------------------------------------------------------------------------
// Topological sort
// ---------------------------------------------------------------------------

#[rstest]
fn topological_sort_empty_graph() {
	// Arrange
	let graph = MigrationGraph::new();

	// Act
	let order = graph.topological_sort().unwrap();

	// Assert
	assert!(order.is_empty());
}

#[rstest]
fn topological_sort_single_node() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let key = MigrationKey::new("auth", "0001_initial");
	graph.add_migration(key.clone(), vec![]);

	// Act
	let order = graph.topological_sort().unwrap();

	// Assert
	assert_eq!(order.len(), 1);
	assert_eq!(order[0], key);
}

#[rstest]
fn topological_sort_linear_chain() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let key1 = MigrationKey::new("auth", "0001_initial");
	let key2 = MigrationKey::new("auth", "0002_add_field");
	let key3 = MigrationKey::new("auth", "0003_alter_field");
	graph.add_migration(key1.clone(), vec![]);
	graph.add_migration(key2.clone(), vec![key1.clone()]);
	graph.add_migration(key3.clone(), vec![key2.clone()]);

	// Act
	let order = graph.topological_sort().unwrap();

	// Assert
	assert_eq!(order.len(), 3);
	assert_eq!(order[0], key1);
	assert_eq!(order[1], key2);
	assert_eq!(order[2], key3);
}

#[rstest]
fn topological_sort_diamond_dependency() {
	// Arrange
	//    A
	//   / \
	//  B   C
	//   \ /
	//    D
	let mut graph = MigrationGraph::new();
	let a = MigrationKey::new("app", "a");
	let b = MigrationKey::new("app", "b");
	let c = MigrationKey::new("app", "c");
	let d = MigrationKey::new("app", "d");
	graph.add_migration(a.clone(), vec![]);
	graph.add_migration(b.clone(), vec![a.clone()]);
	graph.add_migration(c.clone(), vec![a.clone()]);
	graph.add_migration(d.clone(), vec![b.clone(), c.clone()]);

	// Act
	let order = graph.topological_sort().unwrap();

	// Assert - A must come first, D must come last, B and C in between
	assert_eq!(order.len(), 4);
	let pos_a = order.iter().position(|k| k == &a).unwrap();
	let pos_b = order.iter().position(|k| k == &b).unwrap();
	let pos_c = order.iter().position(|k| k == &c).unwrap();
	let pos_d = order.iter().position(|k| k == &d).unwrap();
	assert!(pos_a < pos_b);
	assert!(pos_a < pos_c);
	assert!(pos_b < pos_d);
	assert!(pos_c < pos_d);
}

#[rstest]
fn topological_sort_multi_app() {
	// Arrange - auth.0001 -> users.0001 -> users.0002
	let mut graph = MigrationGraph::new();
	let auth1 = MigrationKey::new("auth", "0001_initial");
	let users1 = MigrationKey::new("users", "0001_initial");
	let users2 = MigrationKey::new("users", "0002_add_profile");
	graph.add_migration(auth1.clone(), vec![]);
	graph.add_migration(users1.clone(), vec![auth1.clone()]);
	graph.add_migration(users2.clone(), vec![users1.clone()]);

	// Act
	let order = graph.topological_sort().unwrap();

	// Assert
	assert_eq!(order.len(), 3);
	let pos_auth1 = order.iter().position(|k| k == &auth1).unwrap();
	let pos_users1 = order.iter().position(|k| k == &users1).unwrap();
	let pos_users2 = order.iter().position(|k| k == &users2).unwrap();
	assert!(pos_auth1 < pos_users1);
	assert!(pos_users1 < pos_users2);
}

#[rstest]
fn topological_sort_circular_dependency_returns_err() {
	// Arrange - A -> B -> C -> A (cycle)
	let mut graph = MigrationGraph::new();
	let a = MigrationKey::new("app", "a");
	let b = MigrationKey::new("app", "b");
	let c = MigrationKey::new("app", "c");
	graph.add_migration(a.clone(), vec![c.clone()]);
	graph.add_migration(b.clone(), vec![a.clone()]);
	graph.add_migration(c.clone(), vec![b.clone()]);

	// Act
	let result = graph.topological_sort();

	// Assert
	assert!(result.is_err());
}

#[rstest]
fn topological_sort_external_deps_ignored() {
	// Arrange - key2 depends on an external key that is not in the graph
	let mut graph = MigrationGraph::new();
	let key1 = MigrationKey::new("app", "0001_initial");
	let external = MigrationKey::new("external", "0001_initial");
	let key2 = MigrationKey::new("app", "0002_add_field");
	graph.add_migration(key1.clone(), vec![]);
	graph.add_migration(key2.clone(), vec![key1.clone(), external.clone()]);

	// Act - external deps should not cause failure
	let order = graph.topological_sort().unwrap();

	// Assert
	assert_eq!(order.len(), 2);
	let pos1 = order.iter().position(|k| k == &key1).unwrap();
	let pos2 = order.iter().position(|k| k == &key2).unwrap();
	assert!(pos1 < pos2);
}

// ---------------------------------------------------------------------------
// Leaf and root nodes
// ---------------------------------------------------------------------------

#[rstest]
fn get_leaf_nodes_returns_nodes_with_no_dependents() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let key1 = MigrationKey::new("auth", "0001_initial");
	let key2 = MigrationKey::new("auth", "0002_add_field");
	graph.add_migration(key1.clone(), vec![]);
	graph.add_migration(key2.clone(), vec![key1.clone()]);

	// Act
	let leaves = graph.get_leaf_nodes();

	// Assert
	assert_eq!(leaves.len(), 1);
	assert_eq!(leaves[0], &key2);
}

#[rstest]
fn get_root_nodes_returns_nodes_with_no_dependencies() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let key1 = MigrationKey::new("auth", "0001_initial");
	let key2 = MigrationKey::new("auth", "0002_add_field");
	graph.add_migration(key1.clone(), vec![]);
	graph.add_migration(key2.clone(), vec![key1.clone()]);

	// Act
	let roots = graph.get_root_nodes();

	// Assert
	assert_eq!(roots.len(), 1);
	assert_eq!(roots[0], &key1);
}

#[rstest]
fn get_leaf_nodes_multiple_leaves() {
	// Arrange - two independent chains: A->B, C->D
	let mut graph = MigrationGraph::new();
	let a = MigrationKey::new("app1", "a");
	let b = MigrationKey::new("app1", "b");
	let c = MigrationKey::new("app2", "c");
	let d = MigrationKey::new("app2", "d");
	graph.add_migration(a.clone(), vec![]);
	graph.add_migration(b.clone(), vec![a.clone()]);
	graph.add_migration(c.clone(), vec![]);
	graph.add_migration(d.clone(), vec![c.clone()]);

	// Act
	let leaves = graph.get_leaf_nodes();

	// Assert
	assert_eq!(leaves.len(), 2);
	assert!(leaves.contains(&&b));
	assert!(leaves.contains(&&d));
}

#[rstest]
fn get_root_nodes_multiple_roots() {
	// Arrange - two independent chains: A->B, C->D
	let mut graph = MigrationGraph::new();
	let a = MigrationKey::new("app1", "a");
	let b = MigrationKey::new("app1", "b");
	let c = MigrationKey::new("app2", "c");
	let d = MigrationKey::new("app2", "d");
	graph.add_migration(a.clone(), vec![]);
	graph.add_migration(b.clone(), vec![a.clone()]);
	graph.add_migration(c.clone(), vec![]);
	graph.add_migration(d.clone(), vec![c.clone()]);

	// Act
	let roots = graph.get_root_nodes();

	// Assert
	assert_eq!(roots.len(), 2);
	assert!(roots.contains(&&a));
	assert!(roots.contains(&&c));
}

// ---------------------------------------------------------------------------
// Squashed migrations (replaces)
// ---------------------------------------------------------------------------

#[rstest]
fn add_migration_with_replaces_stores_replaces() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let squashed = MigrationKey::new("auth", "0001_squashed_0003");
	let old1 = MigrationKey::new("auth", "0001_initial");
	let old2 = MigrationKey::new("auth", "0002_add_field");

	// Act
	graph.add_migration_with_replaces(squashed.clone(), vec![], vec![old1.clone(), old2.clone()]);

	// Assert
	assert!(graph.has_migration(&squashed));
	let node = graph.get_node(&squashed).unwrap();
	assert_eq!(node.replaces.len(), 2);
}

#[rstest]
fn is_replaced_detects_replaced_migrations() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let old = MigrationKey::new("auth", "0001_initial");
	let squashed = MigrationKey::new("auth", "0001_squashed");
	graph.add_migration_with_replaces(squashed.clone(), vec![], vec![old.clone()]);

	// Act

	// Assert
	assert!(graph.is_replaced(&old));
	assert!(!graph.is_replaced(&squashed));
}

#[rstest]
fn get_replacement_returns_replacing_migration() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let old = MigrationKey::new("auth", "0001_initial");
	let squashed = MigrationKey::new("auth", "0001_squashed");
	graph.add_migration_with_replaces(squashed.clone(), vec![], vec![old.clone()]);

	// Act
	let replacement = graph.get_replacement(&old);

	// Assert
	assert_eq!(replacement, Some(&squashed));
}

#[rstest]
fn get_replacement_returns_none_for_non_replaced() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let key = MigrationKey::new("auth", "0001_initial");
	graph.add_migration(key.clone(), vec![]);

	// Act
	let replacement = graph.get_replacement(&key);

	// Assert
	assert!(replacement.is_none());
}

#[rstest]
fn resolve_execution_order_with_replaces_excludes_replaced() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let old1 = MigrationKey::new("auth", "0001_initial");
	let old2 = MigrationKey::new("auth", "0002_add_field");
	let squashed = MigrationKey::new("auth", "0001_squashed_0002");

	graph.add_migration(old1.clone(), vec![]);
	graph.add_migration(old2.clone(), vec![old1.clone()]);
	graph.add_migration_with_replaces(squashed.clone(), vec![], vec![old1.clone(), old2.clone()]);

	// Act
	let order = graph.resolve_execution_order_with_replaces().unwrap();

	// Assert - only the squashed migration should remain
	assert_eq!(order.len(), 1);
	assert_eq!(order[0], squashed);
}

// ---------------------------------------------------------------------------
// find_migration_path
// ---------------------------------------------------------------------------

#[rstest]
fn find_migration_path_linear_chain() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let key1 = MigrationKey::new("auth", "0001_initial");
	let key2 = MigrationKey::new("auth", "0002_add_field");
	let key3 = MigrationKey::new("auth", "0003_alter_field");
	graph.add_migration(key1.clone(), vec![]);
	graph.add_migration(key2.clone(), vec![key1.clone()]);
	graph.add_migration(key3.clone(), vec![key2.clone()]);

	// Act
	let path = graph.find_migration_path(&key1, &key3).unwrap();

	// Assert
	assert_eq!(path.len(), 3);
	assert_eq!(path[0], key1);
	assert_eq!(path[1], key2);
	assert_eq!(path[2], key3);
}

#[rstest]
fn find_migration_path_same_node_returns_single() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let key = MigrationKey::new("auth", "0001_initial");
	graph.add_migration(key.clone(), vec![]);

	// Act
	let path = graph.find_migration_path(&key, &key).unwrap();

	// Assert
	assert_eq!(path.len(), 1);
	assert_eq!(path[0], key);
}

#[rstest]
fn find_migration_path_nonexistent_source_returns_err() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let existing = MigrationKey::new("auth", "0001_initial");
	let missing = MigrationKey::new("auth", "9999_missing");
	graph.add_migration(existing.clone(), vec![]);

	// Act
	let result = graph.find_migration_path(&missing, &existing);

	// Assert
	assert!(result.is_err());
}

#[rstest]
fn find_migration_path_nonexistent_target_returns_err() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let existing = MigrationKey::new("auth", "0001_initial");
	let missing = MigrationKey::new("auth", "9999_missing");
	graph.add_migration(existing.clone(), vec![]);

	// Act
	let result = graph.find_migration_path(&existing, &missing);

	// Assert
	assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// find_backward_path
// ---------------------------------------------------------------------------

#[rstest]
fn find_backward_path_linear_chain() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let key1 = MigrationKey::new("auth", "0001_initial");
	let key2 = MigrationKey::new("auth", "0002_add_field");
	let key3 = MigrationKey::new("auth", "0003_alter_field");
	graph.add_migration(key1.clone(), vec![]);
	graph.add_migration(key2.clone(), vec![key1.clone()]);
	graph.add_migration(key3.clone(), vec![key2.clone()]);

	// Act
	let path = graph.find_backward_path(&key3, &key1).unwrap();

	// Assert
	assert_eq!(path.len(), 3);
	assert_eq!(path[0], key3);
	assert_eq!(path[1], key2);
	assert_eq!(path[2], key1);
}

#[rstest]
fn find_backward_path_same_node_returns_single() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let key = MigrationKey::new("auth", "0001_initial");
	graph.add_migration(key.clone(), vec![]);

	// Act
	let path = graph.find_backward_path(&key, &key).unwrap();

	// Assert
	assert_eq!(path.len(), 1);
	assert_eq!(path[0], key);
}

// ---------------------------------------------------------------------------
// detect_all_cycles
// ---------------------------------------------------------------------------

#[rstest]
fn detect_all_cycles_no_cycle() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let key1 = MigrationKey::new("auth", "0001_initial");
	let key2 = MigrationKey::new("auth", "0002_add_field");
	graph.add_migration(key1.clone(), vec![]);
	graph.add_migration(key2.clone(), vec![key1.clone()]);

	// Act
	let cycles = graph.detect_all_cycles();

	// Assert
	assert!(cycles.is_empty());
}

#[rstest]
fn detect_all_cycles_with_cycle() {
	// Arrange - A -> B -> A (cycle)
	let mut graph = MigrationGraph::new();
	let a = MigrationKey::new("app", "a");
	let b = MigrationKey::new("app", "b");
	graph.add_migration(a.clone(), vec![b.clone()]);
	graph.add_migration(b.clone(), vec![a.clone()]);

	// Act
	let cycles = graph.detect_all_cycles();

	// Assert
	assert!(!cycles.is_empty());
}

// ---------------------------------------------------------------------------
// Multi-app complex graph
// ---------------------------------------------------------------------------

#[rstest]
fn multi_app_complex_graph_with_cross_app_deps() {
	// Arrange
	// auth: 0001_initial
	// users: 0001_initial (depends on auth.0001)
	// users: 0002_add_profile (depends on users.0001)
	// posts: 0001_initial (depends on users.0001)
	// posts: 0002_add_tags (depends on posts.0001)
	let mut graph = MigrationGraph::new();
	let auth1 = MigrationKey::new("auth", "0001_initial");
	let users1 = MigrationKey::new("users", "0001_initial");
	let users2 = MigrationKey::new("users", "0002_add_profile");
	let posts1 = MigrationKey::new("posts", "0001_initial");
	let posts2 = MigrationKey::new("posts", "0002_add_tags");

	graph.add_migration(auth1.clone(), vec![]);
	graph.add_migration(users1.clone(), vec![auth1.clone()]);
	graph.add_migration(users2.clone(), vec![users1.clone()]);
	graph.add_migration(posts1.clone(), vec![users1.clone()]);
	graph.add_migration(posts2.clone(), vec![posts1.clone()]);

	// Act
	let order = graph.topological_sort().unwrap();

	// Assert - verify ordering constraints
	assert_eq!(order.len(), 5);

	let pos = |k: &MigrationKey| order.iter().position(|x| x == k).unwrap();
	assert!(pos(&auth1) < pos(&users1));
	assert!(pos(&users1) < pos(&users2));
	assert!(pos(&users1) < pos(&posts1));
	assert!(pos(&posts1) < pos(&posts2));

	// Verify root and leaf nodes
	let roots = graph.get_root_nodes();
	assert_eq!(roots.len(), 1);
	assert_eq!(roots[0], &auth1);

	let leaves = graph.get_leaf_nodes();
	assert_eq!(leaves.len(), 2);
	assert!(leaves.contains(&&users2));
	assert!(leaves.contains(&&posts2));
}

#[rstest]
fn find_path_across_apps() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let auth1 = MigrationKey::new("auth", "0001_initial");
	let users1 = MigrationKey::new("users", "0001_initial");
	let posts1 = MigrationKey::new("posts", "0001_initial");
	graph.add_migration(auth1.clone(), vec![]);
	graph.add_migration(users1.clone(), vec![auth1.clone()]);
	graph.add_migration(posts1.clone(), vec![users1.clone()]);

	// Act
	let path = graph.find_migration_path(&auth1, &posts1).unwrap();

	// Assert
	assert_eq!(path.len(), 3);
	assert_eq!(path[0], auth1);
	assert_eq!(path[1], users1);
	assert_eq!(path[2], posts1);
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[rstest]
fn graph_independent_nodes_all_roots_and_leaves() {
	// Arrange - three independent nodes with no dependencies
	let mut graph = MigrationGraph::new();
	let a = MigrationKey::new("app1", "0001");
	let b = MigrationKey::new("app2", "0001");
	let c = MigrationKey::new("app3", "0001");
	graph.add_migration(a.clone(), vec![]);
	graph.add_migration(b.clone(), vec![]);
	graph.add_migration(c.clone(), vec![]);

	// Act
	let roots = graph.get_root_nodes();
	let leaves = graph.get_leaf_nodes();
	let order = graph.topological_sort().unwrap();

	// Assert - all nodes are both roots and leaves
	assert_eq!(roots.len(), 3);
	assert_eq!(leaves.len(), 3);
	assert_eq!(order.len(), 3);
}

#[rstest]
fn find_migration_path_no_path_returns_err() {
	// Arrange - two disconnected nodes
	let mut graph = MigrationGraph::new();
	let a = MigrationKey::new("app1", "0001");
	let b = MigrationKey::new("app2", "0001");
	graph.add_migration(a.clone(), vec![]);
	graph.add_migration(b.clone(), vec![]);

	// Act - no forward path from a to b (they are independent)
	let result = graph.find_migration_path(&a, &b);

	// Assert
	assert!(result.is_err());
}

#[rstest]
fn remove_migration_does_not_affect_others() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let key1 = MigrationKey::new("auth", "0001_initial");
	let key2 = MigrationKey::new("auth", "0002_add_field");
	graph.add_migration(key1.clone(), vec![]);
	graph.add_migration(key2.clone(), vec![key1.clone()]);

	// Act
	graph.remove_migration(&key1);

	// Assert - key2 still exists, key1 is gone
	assert!(!graph.has_migration(&key1));
	assert!(graph.has_migration(&key2));
	assert_eq!(graph.len(), 1);
}

#[rstest]
fn get_dependencies_returns_none_for_missing_key() {
	// Arrange
	let graph = MigrationGraph::new();
	let key = MigrationKey::new("auth", "0001_initial");

	// Act
	let deps = graph.get_dependencies(&key);

	// Assert
	assert!(deps.is_none());
}

#[rstest]
fn get_dependents_returns_empty_for_leaf() {
	// Arrange
	let mut graph = MigrationGraph::new();
	let key1 = MigrationKey::new("auth", "0001_initial");
	let key2 = MigrationKey::new("auth", "0002_add_field");
	graph.add_migration(key1.clone(), vec![]);
	graph.add_migration(key2.clone(), vec![key1.clone()]);

	// Act
	let dependents = graph.get_dependents(&key2);

	// Assert
	assert!(dependents.is_empty());
}
