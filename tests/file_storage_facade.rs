use reinhardt::db::orm::FileField;

#[rstest::rstest]
fn file_storage_facade_exposes_typed_file_fields() {
	let file = FileField::from_existing("files/report.txt", "default").unwrap();

	assert_eq!(file.path(), "files/report.txt");
	assert_eq!(file.storage_alias(), "default");
}
