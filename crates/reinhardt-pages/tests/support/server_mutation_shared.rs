use std::marker::PhantomData;

use reinhardt_core::model_form::{
	ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPayload, ModelFormPayloadError,
	ModelFormPolicy, ModelFormSchema, NativeModelFormPayload,
};
use reinhardt_pages::component::{Component, IntoPage, Page, PageElement};
use reinhardt_pages::form;
use reinhardt_pages::server_fn::{ServerFnError, server_fn};
use reinhardt_pages::{
	ClientForm, QueryFamily, ServerMutation, use_form, use_server_mutation,
};
use serde::{Deserialize, Serialize};

pub const CLUSTER_MUTATIONS_READY_ID: &str = "cluster-mutations-ready";
pub const CLUSTER_DETAIL_QUERY: QueryFamily<String, String, String> =
	QueryFamily::new("tests.server-mutation.detail.v1");
pub const CLUSTER_LIST_QUERY: QueryFamily<(), Vec<String>, String> =
	QueryFamily::new("tests.server-mutation.list.v1");

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CreateClusterResponse {
	pub token: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UpdateClusterResponse {
	pub cluster_id: String,
	pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentOrg(pub String);

#[cfg(native)]
#[async_trait::async_trait]
impl reinhardt_di::Injectable for CurrentOrg {
	async fn inject(_ctx: &reinhardt_di::InjectionContext) -> reinhardt_di::DiResult<Self> {
		Ok(Self("org-1".to_owned()))
	}
}

pub struct Cluster;

pub struct ClusterCreatePolicy;

impl ModelFormPolicy for ClusterCreatePolicy {
	fn allows(field: &str) -> bool {
		field == "name"
	}
}

pub struct ClusterFormSchema;

const CLUSTER_CREATE_FIELDS: [ModelFormFieldDescriptor; 1] = [ModelFormFieldDescriptor {
	name: "name",
	kind: ModelFormFieldKind::Text {
		min_length: Some(1),
		max_length: Some(200),
		multiline: false,
	},
	required: true,
	has_default: false,
	nullable: false,
	editable: true,
	generated_relation_id: false,
}];

impl ModelFormSchema for ClusterFormSchema {
	type Model = Cluster;

	fn fields() -> &'static [ModelFormFieldDescriptor] {
		&CLUSTER_CREATE_FIELDS
	}
}

impl ClusterFormSchema {
	pub const fn name() -> &'static ModelFormFieldDescriptor {
		&CLUSTER_CREATE_FIELDS[0]
	}
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(bound = "")]
pub struct ClusterModelFormData<P: ModelFormPolicy> {
	pub name: Option<String>,
	#[serde(skip)]
	pub _policy: PhantomData<P>,
}

impl<P: ModelFormPolicy> Default for ClusterModelFormData<P> {
	fn default() -> Self {
		Self {
			name: None,
			_policy: PhantomData,
		}
	}
}

impl<P: ModelFormPolicy> ModelFormPayload<P> for ClusterModelFormData<P> {
	fn supplied_fields(&self) -> Vec<&'static str> {
		self.name.as_ref().map_or_else(Vec::new, |_| vec!["name"])
	}

	fn forbidden_fields(&self) -> &[&'static str] {
		&[]
	}

	fn get_json(&self, field: &str) -> Option<serde_json::Value> {
		match field {
			"name" => self.name.clone().map(serde_json::Value::String),
			_ => None,
		}
	}

	fn set_json(
		&mut self,
		field: &str,
		value: serde_json::Value,
	) -> Result<(), ModelFormPayloadError> {
		if !P::allows(field) {
			return Err(ModelFormPayloadError::ForbiddenField {
				field: field.to_owned(),
			});
		}
		match (field, value) {
			("name", serde_json::Value::String(name)) => {
				self.name = Some(name);
				Ok(())
			}
			("name", _) => Err(ModelFormPayloadError::InvalidValue {
				field: field.to_owned(),
				message: "name must be a string".to_owned(),
			}),
			(_, _) => Err(ModelFormPayloadError::UnknownField {
				field: field.to_owned(),
			}),
		}
	}
}

impl<P: ModelFormPolicy> NativeModelFormPayload for ClusterModelFormData<P> {
	fn from_native_form_value(value: serde_json::Value) -> Result<Self, serde_json::Error> {
		serde_json::from_value(value)
	}
}

#[server_fn(model_form = true)]
pub async fn create_cluster(
	payload: ClusterModelFormData<ClusterCreatePolicy>,
) -> Result<CreateClusterResponse, ServerFnError> {
	let _ = payload;
	Ok(CreateClusterResponse {
		token: "native-unreachable".to_owned(),
	})
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize, ClientForm)]
#[client_form(server_fn = update_cluster)]
pub struct UpdateClusterRequest {
	pub cluster_id: String,
	pub name: String,
}

#[server_fn]
pub async fn update_cluster(
	request: UpdateClusterRequest,
) -> Result<UpdateClusterResponse, ServerFnError> {
	Ok(UpdateClusterResponse {
		cluster_id: request.cluster_id,
		name: request.name,
	})
}

#[server_fn]
pub async fn delete_cluster(
	cluster_id: String,
	#[inject] current_org: CurrentOrg,
) -> Result<(), ServerFnError> {
	let _ = (cluster_id, current_org);
	Ok(())
}

macro_rules! cluster_create_form {
	() => {
		form! {
			name: ClusterCreateForm,
			model: Cluster,
			policy: ClusterCreatePolicy,
			fields: [name],
			server_fn: create_cluster,
		}
	};
}

pub fn default_update_request() -> UpdateClusterRequest {
	UpdateClusterRequest {
		cluster_id: "cluster-1".to_owned(),
		name: String::new(),
	}
}

pub struct MutationStateProbe {
	pub delete: ServerMutation<String, ()>,
}

impl Component for MutationStateProbe {
	fn render(&self) -> Page {
		let _ = self.delete.phase();
		PageElement::new("div")
			.attr("id", "cluster-delete-probe")
			.attr(
				"data-phase",
				if self.delete.is_pending() {
					"pending"
				} else {
					"idle"
				},
			)
			.into_page()
	}

	fn name() -> &'static str {
		"MutationStateProbe"
	}
}

pub struct ClusterMutationComponent;

impl Component for ClusterMutationComponent {
	fn render(&self) -> Page {
		let create_form = cluster_create_form!();
		let create_runtime = use_form(&create_form).build();
		let update_form = UpdateClusterRequestClientForm::new().with_defaults(default_update_request());
		let update_runtime = use_form(&update_form).build();
		let create = create_form.server_mutation(&create_runtime).build();
		let update = update_form.server_mutation(&update_runtime).build();
		let delete = use_server_mutation(delete_cluster::mutation()).build();
		let _ = (create.phase(), update.phase(), delete.phase());

		PageElement::new("section")
			.attr("id", "cluster-mutations")
			.child(
				PageElement::new("div")
					.attr("id", CLUSTER_MUTATIONS_READY_ID)
					.child("ready")
					.into_page(),
			)
			.child(create_form.into_page())
			.child(
				PageElement::new("div")
					.attr("id", "cluster-update-ready")
					.child("update")
					.into_page(),
			)
			.child(MutationStateProbe { delete }.render())
			.into_page()
	}

	fn name() -> &'static str {
		"ClusterMutationComponent"
	}
}
