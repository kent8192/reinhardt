include!("../pass/model_crud_types.inc");

pub struct MissingMappingResource;
impl ServerFnResource for MissingMappingResource {
	type Lookup = i64;
	type Read = ArticleDto;
	type Create = PublishArticle;
	type Update = UpdateArticle;
	type Patch = PatchArticle;
	type ListQuery = ListQuery;
}
#[async_trait::async_trait]
impl ModelServerFnResource for MissingMappingResource {
	type Model = Article;
	type Policy = AllowAllPolicy;
	fn lookup_field() -> UniqueFieldRef<Article, i64> {
		// SAFETY: The handwritten test model declares `id` as its unique primary key.
		unsafe { UniqueFieldRef::from_model_field("id", "id") }
	}
	async fn to_read(model: &Article, _: Option<&mut dyn TransactionExecutor>) -> Result<ArticleDto, ServerFnSetError> {
		Ok(ArticleDto { id: model.id.unwrap_or_default(), title: model.title.clone() })
	}
}

#[server_fnset(name = "missing-mapping")]
pub fn missing_mapping_fns() -> ModelServerFnSet<MissingMappingResource> { ModelServerFnSet::new() }
fn main() {}
