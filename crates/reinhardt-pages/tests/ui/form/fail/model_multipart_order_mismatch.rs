include!("../model_multipart_support.rs");

use reinhardt_pages::form::{
	ModelFormSelectionArgument, ModelFormSelectionCount, ModelFormServerFn,
};

struct ReversedSelection;

impl ModelFormSelectionCount<3> for ReversedSelection {}

impl ModelFormSelectionArgument<0> for ReversedSelection {
	type Name = upload::__args::document;
}

impl ModelFormSelectionArgument<1> for ReversedSelection {
	type Name = upload::__args::title;
}

impl ModelFormSelectionArgument<2> for ReversedSelection {
	type Name = upload::__args::avatar;
}

fn require_exact_order()
where
	upload::marker: ModelFormServerFn<ReversedSelection, UploadFormSchema, UploadPolicy>,
{
}

fn main() {
}
