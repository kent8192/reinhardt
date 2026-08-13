use reinhardt::db::orm::ImageField;

#[rstest::rstest]
fn image_fields_facade_exposes_typed_image_fields() {
	let image = ImageField::from_existing("images/avatar.png", "default").unwrap();

	assert_eq!(image.path(), "images/avatar.png");
	assert_eq!(image.storage_alias(), "default");
}
