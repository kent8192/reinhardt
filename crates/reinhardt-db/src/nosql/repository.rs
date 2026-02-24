//! Type-safe ODM Repository for MongoDB documents.
//!
//! The [`Repository`] provides CRUD operations that work with typed documents
//! implementing the [`Document`] trait, rather than raw BSON documents.

#[cfg(feature = "mongodb")]
mod mongodb_impl {
	use std::marker::PhantomData;

	use crate::nosql::backends::mongodb::MongoDBBackend;
	use crate::nosql::document::{Document, IndexModel};
	use crate::nosql::error::{OdmError, OdmResult};
	use crate::nosql::traits::DocumentBackend;
	use crate::nosql::types::FindOptions;
	use bson::Document as BsonDocument;

	/// Type-safe repository for ODM CRUD operations.
	///
	/// `Repository<T>` wraps a [`MongoDBBackend`] and provides typed operations
	/// that automatically handle serialization, deserialization, and validation.
	///
	/// ## Example
	///
	/// ```rust,ignore
	/// use reinhardt_db::nosql::Repository;
	///
	/// let repo = Repository::<User>::new(backend);
	/// let mut user = User { id: None, name: "Alice".into(), email: "alice@example.com".into() };
	/// repo.insert(&mut user).await?;
	/// assert!(user.id().is_some());
	/// ```
	pub struct Repository<T: Document> {
		backend: MongoDBBackend,
		_phantom: PhantomData<T>,
	}

	impl<T: Document> Repository<T> {
		/// Create a new repository for the given backend.
		pub fn new(backend: MongoDBBackend) -> Self {
			Self {
				backend,
				_phantom: PhantomData,
			}
		}

		/// Insert a document, setting its ID after successful insertion.
		///
		/// Runs application-level validation before insertion.
		pub async fn insert(&self, doc: &mut T) -> OdmResult<()> {
			doc.validate()?;

			let bson_doc = bson::serialize_to_document(doc)
				.map_err(|e| OdmError::Serialization(e.to_string()))?;

			let id_str = self
				.backend
				.insert_one(T::COLLECTION_NAME, bson_doc)
				.await
				.map_err(OdmError::from)?;

			// Parse the returned ID string back to the document's Id type.
			// Try ObjectId first, then fall back to string.
			let id: T::Id = if let Ok(oid) = bson::oid::ObjectId::parse_str(&id_str) {
				bson::deserialize_from_bson::<T::Id>(bson::Bson::ObjectId(oid))
					.map_err(|e| OdmError::Serialization(e.to_string()))?
			} else {
				bson::deserialize_from_bson::<T::Id>(bson::Bson::String(id_str))
					.map_err(|e| OdmError::Serialization(e.to_string()))?
			};

			doc.set_id(id);
			Ok(())
		}

		/// Find a document by its primary key ID.
		pub async fn find_by_id(&self, id: &T::Id) -> OdmResult<Option<T>> {
			let id_bson =
				bson::serialize_to_bson(id).map_err(|e| OdmError::Serialization(e.to_string()))?;
			let filter = bson::doc! { "_id": id_bson };
			self.find_one(filter).await
		}

		/// Find a single document matching the given BSON filter.
		pub async fn find_one(&self, filter: BsonDocument) -> OdmResult<Option<T>> {
			let result = self
				.backend
				.find_one(T::COLLECTION_NAME, filter)
				.await
				.map_err(OdmError::from)?;

			match result {
				Some(doc) => {
					let typed: T = bson::deserialize_from_document(doc)?;
					Ok(Some(typed))
				}
				None => Ok(None),
			}
		}

		/// Find multiple documents matching the given BSON filter.
		pub async fn find_many(
			&self,
			filter: BsonDocument,
			options: FindOptions,
		) -> OdmResult<Vec<T>> {
			let results = self
				.backend
				.find_many(T::COLLECTION_NAME, filter, options)
				.await
				.map_err(OdmError::from)?;

			results
				.into_iter()
				.map(|doc| bson::deserialize_from_document::<T>(doc).map_err(OdmError::from))
				.collect()
		}

		/// Update a document (full replacement of fields).
		///
		/// Runs application-level validation before update.
		/// The document must have an ID set.
		pub async fn update(&self, doc: &T) -> OdmResult<()> {
			doc.validate()?;

			let id = doc.id().ok_or(OdmError::NotFound)?;
			let id_bson =
				bson::serialize_to_bson(id).map_err(|e| OdmError::Serialization(e.to_string()))?;
			let filter = bson::doc! { "_id": id_bson };

			let mut bson_doc = bson::serialize_to_document(doc)
				.map_err(|e| OdmError::Serialization(e.to_string()))?;
			// Remove _id from the $set payload to avoid immutable field error
			bson_doc.remove("_id");

			let update = bson::doc! { "$set": bson_doc };

			let result = self
				.backend
				.update_one(T::COLLECTION_NAME, filter, update)
				.await
				.map_err(OdmError::from)?;

			if result.matched_count == 0 {
				return Err(OdmError::NotFound);
			}

			Ok(())
		}

		/// Delete a document by its primary key ID.
		pub async fn delete_by_id(&self, id: &T::Id) -> OdmResult<()> {
			let id_bson =
				bson::serialize_to_bson(id).map_err(|e| OdmError::Serialization(e.to_string()))?;
			let filter = bson::doc! { "_id": id_bson };

			let count = self
				.backend
				.delete_one(T::COLLECTION_NAME, filter)
				.await
				.map_err(OdmError::from)?;

			if count == 0 {
				return Err(OdmError::NotFound);
			}

			Ok(())
		}

		/// Create indexes defined by the document's `indexes()` method.
		pub async fn ensure_indexes(&self) -> OdmResult<()> {
			let indexes: Vec<IndexModel> = T::indexes();
			if indexes.is_empty() {
				return Ok(());
			}

			let collection = self
				.backend
				.database()
				.collection::<BsonDocument>(T::COLLECTION_NAME);

			let mongo_indexes: Vec<mongodb::IndexModel> =
				indexes.into_iter().map(mongodb::IndexModel::from).collect();

			collection
				.create_indexes(mongo_indexes)
				.await
				.map_err(|e| OdmError::BackendError(e.to_string()))?;

			Ok(())
		}

		/// Get a reference to the underlying backend.
		pub fn backend(&self) -> &MongoDBBackend {
			&self.backend
		}
	}
}

#[cfg(feature = "mongodb")]
pub use mongodb_impl::Repository;
