/// Authentication identity trait - replacement for the deprecated [`User`](crate::User) trait.
///
/// Provides identity and authentication status methods. Use with
/// [`BaseUser`](crate::BaseUser)/[`FullUser`](crate::FullUser) +
/// [`PermissionsMixin`](crate::PermissionsMixin) for full user functionality.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::AuthIdentity;
///
/// struct MyUser {
///     id: i64,
///     is_superuser: bool,
/// }
///
/// impl AuthIdentity for MyUser {
///     fn id(&self) -> String { self.id.to_string() }
///     fn is_authenticated(&self) -> bool { true }
///     fn is_admin(&self) -> bool { self.is_superuser }
/// }
///
/// let user = MyUser { id: 1, is_superuser: false };
/// assert!(user.is_authenticated());
/// assert!(!user.is_admin());
/// assert_eq!(user.id(), "1");
/// ```
pub trait AuthIdentity: Send + Sync {
	/// Returns the unique identifier for this user as a string.
	fn id(&self) -> String;

	/// Returns whether this user is authenticated.
	///
	/// For concrete user types, this should always return `true`.
	/// `AnonymousUser` should return `false`.
	fn is_authenticated(&self) -> bool;

	/// Returns whether this user is an administrator.
	fn is_admin(&self) -> bool;
}
