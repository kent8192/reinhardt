//! page! macro with numeric attributes using function calls

use reinhardt_pages::page;

fn get_max_len() -> &'static str {
	"100"
}

fn calculate_rows(base: i32) -> String {
	(base * 2).to_string()
}

fn main() {
	let _valid = page!(|| {
		div {
			input {
				r#type: "text",
				maxlength: get_max_len(),
			}
			input {
				r#type: "number",
				min: 0,
				max: calculate_rows(50),
			}
		}
	});
}
