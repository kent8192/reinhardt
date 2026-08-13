include!("../model_multipart_support.rs");

use reinhardt_pages::form::{
	ModelFormSelectionArgument, ModelFormSelectionCount, ModelFormServerFn,
};

struct ShortSelection;

impl ModelFormSelectionCount<2> for ShortSelection {}

impl ModelFormSelectionArgument<0> for ShortSelection {
	type Name = upload::__args::title;
	const NAME: &'static str = "title";
}

impl ModelFormSelectionArgument<1> for ShortSelection {
	type Name = upload::__args::document;
	const NAME: &'static str = "document";
}

fn require_exact_count()
where
	upload::marker: ModelFormServerFn<ShortSelection, UploadFormSchema, UploadPolicy>,
{
}

fn main() {
}
