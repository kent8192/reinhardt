use reinhardt_macros::model;

include!("../support.rs");

#[model(
	app_label = "measurements",
	form(name = MeasurementCreateForm, fields(count, total, ratio, precise, enabled))
)]
struct Measurement {
	#[field(primary_key = true)]
	id: i64,
	count: i32,
	total: i64,
	ratio: f32,
	precise: f64,
	enabled: std::primitive::bool,
}

fn main() {
	use model_form::{ModelFormContract, ModelFormContractSchema};

	assert_eq!(
		<MeasurementCreateFormSchema as ModelFormContractSchema>::contract_fields().len(),
		5
	);
	assert_eq!(<MeasurementCreateForm as ModelFormContract>::fields().len(), 5);
}
