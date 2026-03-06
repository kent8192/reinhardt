//! Integration tests for reinhardt-taggit
//!
//! Integration tests test the interaction between components and the database.

#[path = "integration/cascade_delete_test.rs"]
mod cascade_delete_test;
#[path = "integration/tag_crud_test.rs"]
mod tag_crud_test;
#[path = "integration/tag_relationship_test.rs"]
mod tag_relationship_test;
#[path = "integration/tagged_item_crud_test.rs"]
mod tagged_item_crud_test;
