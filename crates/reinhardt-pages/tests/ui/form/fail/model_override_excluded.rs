use reinhardt_pages::form;

struct Question;

fn main() {
	let _form = form! {
		name: QuestionForm,
		model: Question,
		policy: QuestionFields,
		exclude: [owner_id],
		overrides: {
			owner_id: {
				label: "Owner",
			},
		},
	};
}
