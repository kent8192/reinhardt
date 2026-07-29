use db::orm::DatabaseField;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct Vector<const N: usize>(String);

impl<const N: usize> DatabaseField for Vector<N> {
	type Storage = String;

	fn encode_database(&self) -> Result<Self::Storage, db::orm::FieldCodecError> {
		Ok(self.0.clone())
	}

	fn decode_database(
		value: Self::Storage,
		_context: &db::orm::FieldCodecContext,
	) -> Result<Self, db::orm::FieldCodecError> {
		Ok(Self(value))
	}
}
