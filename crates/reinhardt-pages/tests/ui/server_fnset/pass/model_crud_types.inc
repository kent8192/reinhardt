use reinhardt_db::orm::{FieldSelector, Manager, Model, TransactionExecutor, UniqueFieldRef};
// Each compile fixture exercises a different subset of this shared model contract.
#[allow(unused_imports)]
use reinhardt_pages::server_fn::{
	AllowAllPolicy, CreateModelInput, DetailActionContext, ModelServerFnResource, ModelServerFnSet,
	PageRequest, PatchModelInput, ServerFnListQuery,
	ServerFnResource, ServerFnSetError, ServerFnSetRegistration, UpdateModelInput, server_fnset,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Article { id: Option<i64>, title: String }
#[derive(Clone)] pub struct ArticleFields;
impl FieldSelector for ArticleFields { fn with_alias(self, _: &str) -> Self { self } }
impl Model for Article {
	type PrimaryKey = i64; type Fields = ArticleFields; type Objects = Manager<Self>;
	fn table_name() -> &'static str { "articles" }
	fn new_fields() -> Self::Fields { ArticleFields }
	fn primary_key(&self) -> Option<i64> { self.id }
	fn set_primary_key(&mut self, value: i64) { self.id = Some(value); }
}
#[derive(Clone, Serialize, Deserialize)] pub struct ListQuery;
impl ServerFnListQuery for ListQuery { fn page_request(&self) -> PageRequest { PageRequest::default() } }
#[derive(Clone, Serialize, Deserialize)] pub struct ArticleDto { id: i64, title: String }
#[derive(Clone, Serialize, Deserialize)] pub struct CreateArticle { title: String }
#[derive(Clone, Serialize, Deserialize)] pub struct UpdateArticle { title: String }
#[derive(Clone, Serialize, Deserialize)] pub struct PatchArticle { title: Option<String> }
#[derive(Clone, Serialize, Deserialize)] pub struct PublishArticle { label: String }
impl CreateModelInput<Article> for CreateArticle { fn build(self) -> Result<Article, ServerFnSetError> { Ok(Article { id: None, title: self.title }) } }
impl UpdateModelInput<Article> for UpdateArticle { fn apply(self, model: &mut Article) -> Result<(), ServerFnSetError> { model.title = self.title; Ok(()) } }
impl PatchModelInput<Article> for PatchArticle { fn apply_patch(self, model: &mut Article) -> Result<(), ServerFnSetError> { if let Some(title) = self.title { model.title = title; } Ok(()) } }
pub struct ArticleResource;
impl ServerFnResource for ArticleResource {
	type Lookup = i64; type Read = ArticleDto; type Create = CreateArticle;
	type Update = UpdateArticle; type Patch = PatchArticle; type ListQuery = ListQuery;
}
#[async_trait::async_trait]
impl ModelServerFnResource for ArticleResource {
	type Model = Article; type Policy = AllowAllPolicy;
	fn lookup_field() -> UniqueFieldRef<Article, i64> {
		// SAFETY: The handwritten test model declares `id` as its unique primary key.
		unsafe { UniqueFieldRef::from_model_field("id") }
	}
	async fn to_read(model: &Article, _: Option<&mut dyn TransactionExecutor>) -> Result<ArticleDto, ServerFnSetError> {
		Ok(ArticleDto { id: model.id.unwrap_or_default(), title: model.title.clone() })
	}
}
