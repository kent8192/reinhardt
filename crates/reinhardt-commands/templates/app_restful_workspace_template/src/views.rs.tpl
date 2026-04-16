//! Views module for {{ app_name }} app (RESTful)
use reinhardt::define_views;

define_views! {
    // Add your view submodules here. Each `pub mod` declaration
    // corresponds to a file under the `views/` directory.
    // The macro automatically re-exports endpoint functions and
    // URL resolvers so that `#[url_patterns]` can discover them.
    //
    // Example:
    //    pub mod login;
    //    pub mod register;
}
