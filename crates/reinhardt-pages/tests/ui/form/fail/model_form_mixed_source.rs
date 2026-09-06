use reinhardt_pages::form;

struct Cluster;

fn main() {
	let _form = form! {
		name: CreateClusterForm,
		model_form: ClusterCreateForm,
		model: Cluster,
		server_fn: create_cluster,
	};
}
