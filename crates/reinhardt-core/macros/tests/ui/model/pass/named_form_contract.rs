include!("../support.rs");

#[reinhardt_macros::model(app_label = "documents")]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Owner {
	#[field(primary_key = true)]
	id: i64,
}

#[reinhardt_macros::model(
	app_label = "documents",
	form(name = DocumentCreateForm, fields(title, published))
)]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, reinhardt_macros::Model)]
pub struct Document {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 100)]
	title: String,
	published: bool,
	#[rel(foreign_key)]
	owner: db::associations::ForeignKeyField<Owner>,
}

fn main() {
	use model_form::{ModelFormContract, ModelFormContractField, ModelFormContractSchema};

	let mut data = DocumentCreateFormData::default();
	data.set_title("Draft".to_owned());
	data.set_published(false);
	assert_eq!(DocumentCreateForm::title().name, "title");
	assert_eq!(DocumentCreateFormField::Published.name(), "published");
	assert_eq!(
		<DocumentCreateFormSchema as ModelFormContractSchema>::contract_fields().len(),
		2
	);
	assert_eq!(<DocumentCreateForm as ModelFormContract>::fields().len(), 2);
}
