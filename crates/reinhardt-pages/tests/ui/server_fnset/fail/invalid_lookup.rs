include!("../pass/model_crud_types.inc");

pub struct InvalidLookupResource;
impl ServerFnResource for InvalidLookupResource {
	type Lookup = String;
	type Read = ArticleDto;
	type Create = CreateArticle;
	type Update = UpdateArticle;
	type Patch = PatchArticle;
	type ListQuery = ListQuery;
}
#[async_trait::async_trait]
impl ModelServerFnResource for InvalidLookupResource {
	type Model = Article;
	type Policy = AllowAllPolicy;
	fn lookup_field() -> UniqueFieldRef<Article, String> {
		// SAFETY: The handwritten test model declares `id` as its unique primary key.
		unsafe { UniqueFieldRef::<Article, i64>::from_model_field_with_names("id", "id") }
	}
	async fn to_read(model: &Article, _: Option<&mut dyn TransactionExecutor>) -> Result<ArticleDto, ServerFnSetError> {
		Ok(ArticleDto { id: model.id.unwrap_or_default(), title: model.title.clone() })
	}
}
fn main() {}
