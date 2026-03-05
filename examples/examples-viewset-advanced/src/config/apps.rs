use reinhardt::installed_apps;

installed_apps! {
	authors: "authors",
	books: "books",
	articles: "articles",
}

pub fn get_installed_apps() -> Vec<String> {
	InstalledApp::all_apps()
}
