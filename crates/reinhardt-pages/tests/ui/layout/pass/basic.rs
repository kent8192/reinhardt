use reinhardt_pages::{Outlet, Page, Path, Query, component, layout, page};
use reinhardt_urls::routers::ClientRouter;

#[layout("/workspaces/{workspace_id}/", name = "workspace-shell")]
fn workspace_shell(
	Path(workspace_id): Path<i64>,
	Query(tab): Query<std::option::Option<String>>,
	outlet: Outlet,
) -> Page {
	page!(|workspace_id: i64, tab: Option<String>, outlet: Outlet| {
		div {
			{ format!("{workspace_id}:{tab:?}") }
			{ outlet }
		}
	})(workspace_id, tab, outlet)
}

#[component("jobs", name = "workspace-jobs")]
fn workspace_jobs(Path(workspace_id): Path<i64>) -> Page {
	page!(|workspace_id: i64| {
		p { { workspace_id.to_string() } }
	})(workspace_id)
}

fn main() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let _ = WorkspaceShellProps::builder()
			.workspace_id(7)
			.outlet(Outlet::inline(Page::empty()))
			.build();
		let _ = ClientRouter::new().routes(|routes| {
			routes.layout(workspace_shell, |children| {
				children.component(workspace_jobs)
			})
		});
	});
}
