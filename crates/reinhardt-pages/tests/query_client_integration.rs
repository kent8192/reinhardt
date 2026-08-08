use reinhardt_pages::reactive::{QueryFamily, QueryOptions, use_query};

#[cfg(all(native, feature = "testing"))]
use std::cell::{Cell, RefCell};
#[cfg(all(native, feature = "testing"))]
use std::rc::Rc;
#[cfg(all(native, feature = "testing"))]
use std::time::Duration;

#[cfg(all(native, feature = "testing"))]
use reinhardt_core::types::page::{IntoPage, Page, PageElement};
#[cfg(all(native, feature = "testing"))]
use reinhardt_pages::reactive::entity::{
	EntityDependencies, EntityProjection, EntityReader, EntityWriter, ProjectionMaterialization,
	ProjectionRemoval, RemovedEntities,
};
#[cfg(all(native, feature = "testing"))]
use reinhardt_pages::reactive::hooks::use_action;
#[cfg(all(native, feature = "testing"))]
use reinhardt_pages::reactive::{
	Entity, QueryClient, QueryHandle, QuerySnapshot, QueryStatus, ReactiveScope, queries,
};
#[cfg(all(native, feature = "testing"))]
use reinhardt_pages::testing::component::{Role, render};
#[cfg(all(native, feature = "testing"))]
use serde::{Deserialize, Serialize};

#[cfg(all(native, feature = "testing"))]
const JOBS: QueryFamily<u64, Vec<String>, String> = QueryFamily::new("acceptance.jobs");

#[cfg(all(native, feature = "testing"))]
const PROJECTS: QueryFamily<u64, ProjectCollection, String> =
	QueryFamily::new("acceptance.normalized-projects");

#[cfg(all(native, feature = "testing"))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct Project {
	id: u64,
	name: String,
}

#[cfg(all(native, feature = "testing"))]
impl Entity for Project {
	type Id = u64;

	const TYPE: &'static str = "acceptance.normalized-project";

	fn entity_id(&self) -> Self::Id {
		self.id
	}
}

#[cfg(all(native, feature = "testing"))]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct ProjectCollection {
	label: String,
	projects: Vec<Project>,
}

#[cfg(all(native, feature = "testing"))]
#[derive(Clone, Deserialize, Serialize)]
struct ProjectCollectionRecipe {
	label: String,
	project_ids: Vec<u64>,
}

#[cfg(all(native, feature = "testing"))]
#[derive(Clone, Copy)]
struct ProjectCollectionProjection;

#[cfg(all(native, feature = "testing"))]
impl EntityProjection<ProjectCollection> for ProjectCollectionProjection {
	type Recipe = ProjectCollectionRecipe;

	const SCHEMA: &'static str = "project-collection-v1";

	fn normalize(&self, value: ProjectCollection, entities: &mut EntityWriter<'_>) -> Self::Recipe {
		let project_ids = value
			.projects
			.into_iter()
			.map(|project| {
				let id = project.id;
				entities.upsert(project);
				id
			})
			.collect();
		ProjectCollectionRecipe {
			label: value.label,
			project_ids,
		}
	}

	fn dependencies(&self, recipe: &Self::Recipe, dependencies: &mut EntityDependencies) {
		dependencies.extend::<Project>(recipe.project_ids.iter().copied());
	}

	fn materialize(
		&self,
		recipe: &Self::Recipe,
		entities: &EntityReader<'_>,
	) -> ProjectionMaterialization<ProjectCollection> {
		match entities.required_vec::<Project>(&recipe.project_ids) {
			ProjectionMaterialization::Ready(projects) => {
				ProjectionMaterialization::Ready(ProjectCollection {
					label: recipe.label.clone(),
					projects,
				})
			}
			ProjectionMaterialization::MissingRequired => {
				ProjectionMaterialization::MissingRequired
			}
		}
	}

	fn apply_removals(
		&self,
		recipe: &mut Self::Recipe,
		removed: &RemovedEntities<'_>,
	) -> ProjectionRemoval {
		let previous_len = recipe.project_ids.len();
		recipe
			.project_ids
			.retain(|id| !removed.contains::<Project>(id));
		ProjectionRemoval::from_changed(previous_len != recipe.project_ids.len())
	}
}

#[cfg(all(native, feature = "testing"))]
fn project(id: u64, name: &str) -> Project {
	Project {
		id,
		name: name.to_string(),
	}
}

#[cfg(all(native, feature = "testing"))]
fn project_collection(label: &str, projects: Vec<Project>) -> ProjectCollection {
	ProjectCollection {
		label: label.to_string(),
		projects,
	}
}

#[cfg(all(native, feature = "testing"))]
fn normalized_project_queries(
	client: Rc<RefCell<Option<QueryClient>>>,
	first: Rc<RefCell<Option<QueryHandle<ProjectCollection, String>>>>,
	second: Rc<RefCell<Option<QueryHandle<ProjectCollection, String>>>>,
	entity_name: &'static str,
) -> Page {
	let query_client = queries();
	client.borrow_mut().replace(query_client.clone());
	first.borrow_mut().replace(use_query(
		PROJECTS
			.query(1, move || async move {
				Ok(project_collection(
					"first recipe",
					vec![project(1, entity_name), project(2, "member")],
				))
			})
			.with_entities(ProjectCollectionProjection),
		QueryOptions::new(),
	));
	second.borrow_mut().replace(use_query(
		PROJECTS
			.query(2, move || async move {
				Ok(project_collection(
					"second recipe",
					vec![project(1, entity_name)],
				))
			})
			.with_entities(ProjectCollectionProjection),
		QueryOptions::new(),
	));
	PageElement::new("p")
		.child("normalized queries")
		.into_page()
}

#[cfg(all(native, feature = "testing"))]
async fn list_jobs(project_id: u64, calls: Rc<Cell<usize>>) -> Result<Vec<String>, String> {
	let call = calls.get() + 1;
	calls.set(call);
	Ok(vec![format!("project {project_id} job {call}")])
}

#[cfg(all(native, feature = "testing"))]
async fn retry_job(project_id: u64, calls: Rc<Cell<usize>>) -> Result<u64, String> {
	calls.set(calls.get() + 1);
	Ok(project_id)
}

#[cfg(all(native, feature = "testing"))]
fn jobs_snapshot_page(label: &'static str, snapshot: QuerySnapshot<Vec<String>, String>) -> Page {
	match snapshot.status {
		QueryStatus::Idle => PageElement::new("p")
			.child(format!("{label}: idle"))
			.into_page(),
		QueryStatus::Pending => PageElement::new("p")
			.child(format!("{label}: loading"))
			.into_page(),
		QueryStatus::Success => PageElement::new("p")
			.child(format!(
				"{label}: {}",
				snapshot
					.data
					.expect("successful jobs snapshots contain data")
					.join(", ")
			))
			.into_page(),
		QueryStatus::Error => PageElement::new("p")
			.child(format!(
				"{label}: {}",
				snapshot
					.error
					.expect("failed jobs snapshots contain an error")
			))
			.into_page(),
	}
}

#[cfg(all(native, feature = "testing"))]
fn jobs_component(project_id: u64, label: &'static str, list_calls: Rc<Cell<usize>>) -> Page {
	let jobs = use_query(
		JOBS.query(project_id, move || {
			list_jobs(project_id, Rc::clone(&list_calls))
		}),
		QueryOptions::new().refetch_interval(Duration::from_secs(5)),
	);

	Page::reactive(move || jobs_snapshot_page(label, jobs.snapshot()))
}

#[cfg(all(native, feature = "testing"))]
fn jobs_screen(project_id: u64, list_calls: Rc<Cell<usize>>, retry_calls: Rc<Cell<usize>>) -> Page {
	let client = queries();
	let retry = use_action(move |request| {
		let client = client.clone();
		let retry_calls = Rc::clone(&retry_calls);
		async move {
			let result = retry_job(request, retry_calls).await?;
			client.invalidate_family(JOBS);
			Ok::<_, String>(result)
		}
	});

	PageElement::new("main")
		.child(
			PageElement::new("button")
				.listener("click", move |_| retry.dispatch(project_id))
				.child("Retry job"),
		)
		.child(jobs_component(project_id, "Queue", Rc::clone(&list_calls)))
		.child(jobs_component(project_id, "Sidebar", list_calls))
		.into_page()
}

#[test]
#[should_panic(expected = "use_query requires an active QueryClient")]
fn use_query_rejects_missing_application_context() {
	let family = QueryFamily::<(), String, String>::new("tests.no-client");
	let _query = use_query(
		family.query((), || async { Ok("value".to_string()) }),
		QueryOptions::default(),
	);
}

#[cfg(all(native, feature = "testing"))]
#[tokio::test]
async fn jobs_screen_deduplicates_shared_reads_and_refetches_after_retry() {
	// Arrange
	let list_calls = Rc::new(Cell::new(0));
	let retry_calls = Rc::new(Cell::new(0));
	let screen = render({
		let list_calls = Rc::clone(&list_calls);
		let retry_calls = Rc::clone(&retry_calls);
		move || jobs_screen(42, list_calls, retry_calls)
	});

	// Act
	screen.settle().await;

	// Assert
	assert_eq!(list_calls.get(), 1);
	assert_eq!(
		screen
			.try_get_by_text("Sidebar: project 42 job 1")
			.expect("the sidebar renders the shared jobs result")
			.text(),
		"Sidebar: project 42 job 1"
	);
	assert_eq!(
		screen
			.try_get_by_text("Queue: project 42 job 1")
			.expect("the queue renders the shared jobs result")
			.text(),
		"Queue: project 42 job 1"
	);

	// Act
	screen.get_by_role(Role::Button, "Retry job").click();
	screen.settle().await;

	// Assert
	assert_eq!(retry_calls.get(), 1);
	assert_eq!(list_calls.get(), 2);
	assert_eq!(
		screen
			.try_get_by_text("Sidebar: project 42 job 2")
			.expect("the sidebar renders the refetched jobs result")
			.text(),
		"Sidebar: project 42 job 2"
	);
	assert_eq!(
		screen
			.try_get_by_text("Queue: project 42 job 2")
			.expect("the queue renders the refetched jobs result")
			.text(),
		"Queue: project 42 job 2"
	);
}

#[cfg(all(native, feature = "testing"))]
#[tokio::test]
async fn public_entity_mutation_propagates_to_exact_queries_in_one_client_only() {
	// Arrange
	let first_client = Rc::new(RefCell::new(None));
	let first_query = Rc::new(RefCell::new(None));
	let first_sibling = Rc::new(RefCell::new(None));
	let first_screen = render({
		let client = Rc::clone(&first_client);
		let first = Rc::clone(&first_query);
		let second = Rc::clone(&first_sibling);
		move || normalized_project_queries(client, first, second, "first client")
	});
	let second_client = Rc::new(RefCell::new(None));
	let second_query = Rc::new(RefCell::new(None));
	let second_sibling = Rc::new(RefCell::new(None));
	let second_screen = render({
		let client = Rc::clone(&second_client);
		let first = Rc::clone(&second_query);
		let second = Rc::clone(&second_sibling);
		move || normalized_project_queries(client, first, second, "second client")
	});
	first_screen.settle().await;
	second_screen.settle().await;
	let first_client = first_client
		.borrow()
		.clone()
		.expect("the first screen captures its query client");
	let second_client = second_client
		.borrow()
		.clone()
		.expect("the second screen captures its query client");
	ReactiveScope::run(|| {
		let first_entity = first_client.entity::<Project>(1);
		let second_entity = second_client.entity::<Project>(1);

		// Act
		first_client.update_entities(|entities| {
			entities.upsert(project(1, "updated"));
			entities.upsert(project(3, "not inferred"));
		});

		// Assert
		assert_eq!(first_entity.get(), Some(project(1, "updated")));
		assert_eq!(second_entity.get(), Some(project(1, "second client")));
		assert_eq!(
			first_query
				.borrow()
				.as_ref()
				.expect("the first exact query remains mounted")
				.data(),
			Some(project_collection(
				"first recipe",
				vec![project(1, "updated"), project(2, "member")],
			))
		);
		assert_eq!(
			first_sibling
				.borrow()
				.as_ref()
				.expect("the sibling exact query remains mounted")
				.data(),
			Some(project_collection(
				"second recipe",
				vec![project(1, "updated")],
			))
		);
		assert_eq!(
			second_query
				.borrow()
				.as_ref()
				.expect("the isolated exact query remains mounted")
				.data(),
			Some(project_collection(
				"first recipe",
				vec![project(1, "second client"), project(2, "member")],
			))
		);
	});
}
