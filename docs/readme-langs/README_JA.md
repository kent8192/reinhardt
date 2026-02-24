<div align="center">
  <img src="../../branding/logo.png" alt="Reinhardt Logo" width="200"/>

  <h1>Reinhardt</h1>

  <h3>🦀 ポリリシック・バッテリー同梱</h3>

  <p><strong>Rust向けコンポーザブルフルスタックAPIフレームワーク</strong></p>
  <p>Djangoの「バッテリー同梱」哲学の<em>すべて</em>のパワーで構築するか、<br/>
  必要なものだけを<em>組み合わせる</em>か—あなたの選択、あなたの方法で。</p>

🌐 [English](../../README.md) | **日本語** | [简体中文](README_ZH_CN.md) | [繁體中文](README_ZH_TW.md) | [Русский](README_RU.md) | [Українська](README_UK.md) | [فارسی](README_FA.md) | [العربية](README_AR.md)

[![Crates.io](https://img.shields.io/crates/v/reinhardt-web.svg)](https://crates.io/crates/reinhardt-web)
[![Documentation](https://docs.rs/reinhardt-web/badge.svg)](https://docs.rs/reinhardt-web)
[![License](https://img.shields.io/badge/license-BSD--3--Clause-blue.svg)](../../LICENSE)
[![codecov](https://codecov.io/gh/kent8192/reinhardt-web/graph/badge.svg)](https://codecov.io/gh/kent8192/reinhardt-web)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/kent8192/reinhardt-web)

</div>

---

## 📍 クイックナビゲーション

お探しの情報:

- 🚀 [クイックスタート](#クイックスタート) - 5分で起動
- 📦 [インストールオプション](#インストール) - フレーバーを選択: Micro、Standard、Full
- 📚 [はじめに](../GETTING_STARTED.md) - ステップバイステップチュートリアル
- 🎛️ [機能フラグ](../FEATURE_FLAGS.md) - ビルドを最適化
- 📖 [APIドキュメント](https://docs.rs/reinhardt-web) - 完全なAPIリファレンス
- 💬 [コミュニティ＆サポート](#ヘルプを得る) - コミュニティからサポートを受ける

## なぜReinhardtか?

**Polylithic = Poly（多数）+ Lithic（構成要素）**
すべてを使用することを強制するモノリシックフレームワークとは異なり、Reinhardtは独立した、十分にテストされたコンポーネントから完璧なスタックを構成できます。

Reinhardtは3つの世界のベストを統合しています:

| インスピレーション    | 借用したもの                                         | 改善したもの                                      |
|--------------------|--------------------------------------------------------|------------------------------------------------------|
| 🐍 **Django**      | バッテリー同梱哲学、ORM設計、管理パネル                   | コンポーザブルビルドのための機能フラグ、Rustの型安全性     |
| 🎯 **Django REST** | シリアライザー、ViewSets、パーミッション                   | コンパイル時検証、ゼロコスト抽象化                      |
| ⚡ **FastAPI**      | DIシステム、自動OpenAPI                                 | ネイティブRustパフォーマンス、ランタイムオーバーヘッドなし   |
| 🗄️ **SQLAlchemy** | QuerySetパターン、リレーションシップ処理                   | 型安全なクエリビルダー、コンパイル時検証                 |

**結果**: Pythonデベロッパーに馴染みやすく、Rustのパフォーマンスと安全性保証を持つフレームワーク。

## ✨ 主な機能

- **型安全なORM** コンパイル時検証付き（reinhardt-query）
- **強力なシリアライザー** 自動検証付き（serde + validator）
- **FastAPIスタイルのDI** 型安全な依存性注入とキャッシング
- **ViewSets** 迅速なCRUD API開発用
- **マルチ認証**（JWT、Token、Session、Basic）BaseUser/FullUserトレイト付き
- **管理パネル** 自動生成されたモデル管理インターフェース
- **管理コマンド** マイグレーション、静的ファイルなど
- **GraphQL＆WebSocket** リアルタイムアプリケーション対応
- **ページネーション、フィルタリング、レート制限** 組み込み
- **シグナル** イベント駆動アーキテクチャ用

完全なリストは[利用可能なコンポーネント](#利用可能なコンポーネント)を、例は[はじめに](../GETTING_STARTED.md)を参照してください。

## インストール

Reinhardtはモジュラーフレームワークです。出発点を選択してください:

**クレート命名に関する注意:**
メインのReinhardtクレートはcrates.ioに`reinhardt-web`として公開されていますが、`package`属性を使用してコード内では`reinhardt`としてインポートします。

### デフォルト: フル機能（バッテリー同梱）⚠️ 新しいデフォルト

設定不要ですべての機能を取得:

```toml
[dependencies]
# 'reinhardt'としてインポート、'reinhardt-web'として公開
# デフォルトですべての機能を有効化（フルバンドル）
reinhardt = { version = "0.1.0-alpha.1", package = "reinhardt-web" }
```

**含まれるもの:** Database、Auth、REST API、Admin、GraphQL、WebSockets、Cache、i18n、Mail、Sessions、Static Files、Storage

**バイナリ**: ~50+ MB | **コンパイル**: 遅いが、すべてがすぐに動作

コードでの使用:
```rust
use reinhardt::prelude::*;
use reinhardt::{Request, Response, StatusCode};
```

### オプション1: 標準セットアップ（バランス型）

すべての機能が必要ないほとんどのプロジェクト向け:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", package = "reinhardt-web", default-features = false, features = ["standard"] }
```

**含まれるもの:** Core、Database（PostgreSQL）、REST API、Auth、Middleware、Pages（SSR付きWASMフロントエンド）

**バイナリ**: ~20-30 MB | **コンパイル**: 中程度

### オプション2: マイクロサービス（最小セットアップ）

軽量で高速、シンプルなAPI向け:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", package = "reinhardt-web", default-features = false, features = ["minimal"] }
```

**含まれるもの:** HTTP、ルーティング、DI、パラメータ抽出、サーバー

**バイナリ**: ~5-10 MB | **コンパイル**: 非常に高速

### オプション3: カスタムスタックを構築

必要なコンポーネントのみをインストール:

```toml
[dependencies]
# コアコンポーネント
reinhardt-http = "0.1.0-alpha.1"
reinhardt-urls = "0.1.0-alpha.1"

# オプション: データベース
reinhardt-db = "0.1.0-alpha.1"

# オプション: 認証
reinhardt-auth = "0.1.0-alpha.1"

# オプション: REST API機能
reinhardt-rest = "0.1.0-alpha.1"

# オプション: 管理パネル
reinhardt-admin = "0.1.0-alpha.1"

# オプション: 高度な機能
reinhardt-graphql = "0.1.0-alpha.1"
reinhardt-websockets = "0.1.0-alpha.1"
```

**📖 利用可能なクレートと機能フラグの完全なリストは、[機能フラグガイド](../FEATURE_FLAGS.md)を参照してください。**

## クイックスタート

### 1. Reinhardt管理ツールをインストール

```bash
cargo install reinhardt-admin-cli
```

### 2. 新しいプロジェクトを作成

```bash
# RESTful APIプロジェクトを作成（デフォルト）
reinhardt-admin startproject my-api
cd my-api
```

これにより完全なプロジェクト構造が生成されます:

```
my-api/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── config.rs
│   ├── apps.rs
│   ├── config/
│   │   ├── settings.rs
│   │   ├── settings/
│   │   │   ├── base.rs
│   │   │   ├── local.rs
│   │   │   ├── staging.rs
│   │   │   └── production.rs
│   │   ├── urls.rs
│   │   └── apps.rs
│   └── bin/
│       └── manage.rs
└── README.md
```

**代替: reinhardt-pagesプロジェクトを作成（WASM + SSR）**

SSR付きのモダンなWASMベースのフロントエンド向け:

```bash
# pagesプロジェクトを作成
reinhardt-admin startproject my-app --with-pages
cd my-app

# WASMビルドツールをインストール（初回のみ）
cargo make install-wasm-tools

# WASMをビルドして開発サーバーを起動
cargo make dev
# http://127.0.0.1:8000/ にアクセス
```

### 3. 開発サーバーを実行

```bash
# manageコマンドを使用
cargo run --bin manage runserver

# サーバーは http://127.0.0.1:8000 で起動
```

**自動リロードサポート:**

コード変更時の自動リロード（baconが必要）:

```bash
# baconをインストール
cargo install --locked bacon

# 自動リロードで実行
bacon runserver

# またはcargo makeを使用
cargo make watch

# テスト用
bacon test
```

### 4. 最初のアプリを作成

```bash
# RESTful APIアプリを作成（デフォルト）
cargo run --bin manage startapp users

# または明示的にタイプを指定
cargo run --bin manage startapp users --restful

# Pagesアプリを作成（WASM + SSR）
cargo run --bin manage startapp dashboard --with-pages
```

これによりアプリ構造が作成されます:

```
users/
├── lib.rs
├── models.rs
├── models/
├── views.rs
├── views/
├── serializers.rs
├── serializers/
├── admin.rs
├── urls.rs
└── tests.rs
```

### 5. ルートを登録

アプリの`urls.rs`を編集:

```rust
// users/urls.rs
use reinhardt::ServerRouter;

use super::views;

pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::list_users)
		.endpoint(views::get_user)
		.endpoint(views::create_user)
}
```

`src/config/urls.rs`にインクルード:

```rust
// src/config/urls.rs
use reinhardt::prelude::*;
use reinhardt::routes;

#[routes]
pub fn routes() -> ServerRouter {
	ServerRouter::new()
		.mount("/api/", users::urls::url_patterns())
}
```

`#[routes]`属性マクロは、`inventory`クレートを介したフレームワークによる検出のために、この関数を自動的に登録します。

**注意:** `reinhardt::prelude`には一般的に使用される型が含まれています。主なエクスポート:

**常に利用可能:**
- コアルーティングとビュー: `Router`、`DefaultRouter`、`ServerRouter`、`View`、`ListView`、`DetailView`
- ViewSets: `ViewSet`、`ModelViewSet`、`ReadOnlyModelViewSet`
- HTTP: `StatusCode`

**機能依存:**
- **`core`機能**: `Request`、`Response`、`Handler`、`Middleware`、シグナル（`post_save`、`pre_save`など）
- **`database`機能**: `Model`、`DatabaseConnection`、`F`、`Q`、`Transaction`、`atomic`、データベース関数（`Concat`、`Upper`、`Lower`、`Now`、`CurrentDate`）、ウィンドウ関数（`Window`、`RowNumber`、`Rank`、`DenseRank`）、制約（`UniqueConstraint`、`CheckConstraint`、`ForeignKeyConstraint`）
- **`auth`機能**: `User`、`UserManager`、`GroupManager`、`Permission`、`ObjectPermission`
- **`minimal`、`standard`、または`di`機能**: `Body`、`Cookie`、`Header`、`Json`、`Path`、`Query`
- **`rest`機能**: シリアライザー、パーサー、ページネーション、スロットリング、バージョニング
- **`admin`機能**: 管理パネルコンポーネント
- **`cache`機能**: `Cache`、`InMemoryCache`
- **`sessions`機能**: `Session`、`AuthenticationMiddleware`

完全なリストは[機能フラグガイド](../FEATURE_FLAGS.md)を参照してください。

完全なステップバイステップガイドは[はじめに](../GETTING_STARTED.md)を参照してください。

## 🎓 例で学ぶ

### データベース使用時

`settings/base.toml`でデータベースを設定:

```toml
debug = true
secret_key = "your-secret-key-for-development"

[database]
engine = "postgresql"
host = "localhost"
port = 5432
name = "mydb"
user = "postgres"
password = "postgres"
```

設定は`src/config/settings.rs`で自動的に読み込まれます:

```rust
// src/config/settings.rs
use reinhardt::conf::settings::builder::SettingsBuilder;
use reinhardt::conf::settings::profile::Profile;
use reinhardt::conf::settings::sources::{DefaultSource, LowPriorityEnvSource, TomlFileSource};
use reinhardt::core::Settings;
use std::env;
use std::path::PathBuf;
use std::str::FromStr;

pub fn get_settings() -> Settings {
	let profile_str = env::var("REINHARDT_ENV").unwrap_or_else(|_| "local".to_string());
	let profile = Profile::from_str(&profile_str).unwrap_or(Profile::Development);

	let base_dir = env::current_dir().expect("Failed to get current directory");
	let settings_dir = base_dir.join("settings");

	let merged = SettingsBuilder::new()
		.profile(profile)
		.add_source(
			DefaultSource::new()
				.with_value("debug", serde_json::Value::Bool(false))
				.with_value("language_code", serde_json::Value::String("en-us".to_string()))
				.with_value("time_zone", serde_json::Value::String("UTC".to_string()))
		)
		.add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
		.add_source(TomlFileSource::new(settings_dir.join("base.toml")))
		.add_source(TomlFileSource::new(settings_dir.join(format!("{}.toml", profile_str))))
		.build()
		.expect("Failed to build settings");

	merged.into_typed().expect("Failed to convert settings to Settings struct")
}
```

**環境変数ソース:**

Reinhardtは異なる優先度を持つ2種類の環境変数ソースを提供します:

- **`EnvSource`**（優先度: 100）- TOMLファイルを上書きする高優先度環境変数
  ```rust
  .add_source(EnvSource::new().with_prefix("REINHARDT_"))
  ```

- **`LowPriorityEnvSource`**（優先度: 40）- TOMLファイルにフォールバックする低優先度環境変数
  ```rust
  .add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
  ```

**優先順位**:
- `EnvSource`使用時: 環境変数 > `{profile}.toml` > `base.toml` > デフォルト
- `LowPriorityEnvSource`使用時（上記表示）: `{profile}.toml` > `base.toml` > 環境変数 > デフォルト

環境変数を常に優先させたい場合（本番デプロイなど）は`EnvSource`を選択してください。
TOMLファイルを主要な設定ソースにしたい場合（開発など）は`LowPriorityEnvSource`を選択してください。

詳細は[設定ドキュメント](../SETTINGS_DOCUMENT.md)を参照してください。

**組み込みDefaultUserの使用:**

Reinhardtはすぐに使える`DefaultUser`実装を提供します（`argon2-hasher`機能が必要）:

```rust
// users/models.rs
use reinhardt::prelude::*;
use reinhardt::DefaultUser;

// DefaultUserをアプリのUserとして再エクスポート
pub type User = DefaultUser;

// DefaultUserには以下が含まれます:
// - id: Uuid（主キー）
// - username: String
// - email: String
// - password_hash: Option<String>
// - first_name: String
// - last_name: String
// - is_active: bool
// - is_staff: bool
// - is_superuser: bool
// - last_login: Option<DateTime<Utc>>
// - date_joined: DateTime<Utc>

// DefaultUserは以下を実装しています:
// - BaseUserトレイト（認証メソッド）
// - FullUserトレイト（完全なユーザー情報）
// - PermissionsMixinトレイト（権限管理）
// - Modelトレイト（データベース操作）
```

**カスタムユーザーモデルの定義:**

カスタムフィールドが必要な場合は、独自のモデルを定義:

```rust
// users/models.rs
use reinhardt::prelude::*;
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

#[model(app_label = "users", table_name = "users")]
pub struct CustomUser {
	#[field(primary_key = true)]
	pub id: i64,

	#[field(max_length = 255)]
	pub email: String,

	#[field(max_length = 100)]
	pub username: String,

	#[field(default = true)]
	pub is_active: bool,

	#[field(auto_now_add = true)]
	pub created_at: DateTime<Utc>,

	// カスタムフィールドを追加
	#[field(max_length = 50, null = true)]
	pub phone_number: Option<String>,
}
```

**Modelアトリビュートマクロ:**

`#[model(...)]`属性は以下を自動生成します:
- `Model`トレイトの実装（`#[derive(Model)]`機能を含む）
- 型安全なフィールドアクセサー: `User::field_email()`、`User::field_username()`など
- グローバルモデルレジストリ登録
- 複合主キーのサポート

**注意:** `#[model(...)]`を使用する場合、`#[derive(Model)]`を別途追加する必要はありません。
`#[model(...)]`属性によって自動的に適用されます。

**フィールド属性:**
- `#[field(primary_key = true)]` - 主キーとしてマーク
- `#[field(max_length = 255)]` - 文字列フィールドの最大長を設定
- `#[field(default = value)]` - デフォルト値を設定
- `#[field(auto_now_add = true)]` - 作成時にタイムスタンプを自動設定
- `#[field(auto_now = true)]` - 保存時にタイムスタンプを自動更新
- `#[field(null = true)]` - NULL値を許可
- `#[field(unique = true)]` - 一意性制約を強制

フィールド属性の完全なリストは[フィールド属性ガイド](../field_attributes.md)を参照してください。

生成されたフィールドアクセサーにより、クエリで型安全なフィールド参照が可能になります:

```rust
// #[model(...)]によってDefaultUserに生成
impl DefaultUser {
	pub const fn field_id() -> FieldRef<DefaultUser, Uuid> { FieldRef::new("id") }
	pub const fn field_username() -> FieldRef<DefaultUser, String> { FieldRef::new("username") }
	pub const fn field_email() -> FieldRef<DefaultUser, String> { FieldRef::new("email") }
	pub const fn field_is_active() -> FieldRef<DefaultUser, bool> { FieldRef::new("is_active") }
	pub const fn field_is_staff() -> FieldRef<DefaultUser, bool> { FieldRef::new("is_staff") }
	pub const fn field_date_joined() -> FieldRef<DefaultUser, DateTime<Utc>> { FieldRef::new("date_joined") }
	// ... その他のフィールド
}
```

**高度なクエリ例:**

```rust
use reinhardt::prelude::*;
use reinhardt::DefaultUser;

// 型安全なフィールド参照を使用したDjangoスタイルのF/Qオブジェクトクエリ
async fn complex_user_query() -> Result<Vec<DefaultUser>, Box<dyn std::error::Error>> {
	// 型安全なフィールド参照を使用したQオブジェクト（生成されたフィールドアクセサーを使用）
	let active_query = Q::new()
		.field("is_active").eq(true)
		.and(Q::new().field("date_joined").gte(Now::new()));

	// 型安全なフィールド参照を使用したデータベース関数
	let email_lower = Lower::new(DefaultUser::field_email().into());
	let username_upper = Upper::new(DefaultUser::field_username().into());

	// フィールドアクセサーを使用した集計
	let user_count = Aggregate::count(DefaultUser::field_id().into());
	let latest_joined = Aggregate::max(DefaultUser::field_date_joined().into());

	// ランキング用ウィンドウ関数
	let rank_by_join_date = Window::new()
		.partition_by(vec![DefaultUser::field_is_active().into()])
		.order_by(vec![(DefaultUser::field_date_joined().into(), "DESC")])
		.function(RowNumber::new());

	todo!("Execute query with these components")
}

// トランザクションサポート
async fn create_user_with_transaction(
	conn: &DatabaseConnection,
	user_data: CreateUserRequest
) -> Result<User, Box<dyn std::error::Error>> {
	// エラー時に自動ロールバックするトランザクション
	transaction(conn, |_tx| async move {
		let user = User::create(user_data).await?;
		log_user_creation(&user).await?;
		Ok(user)
	}).await
}
```

**注意**: ReinhardtはSQL操作にreinhardt-queryを使用しています。`#[derive(Model)]`マクロはModelトレイト実装、型安全なフィールドアクセサー、グローバルモデルレジストリ登録を自動生成します。

`src/config/apps.rs`で登録:

```rust
// src/config/apps.rs
use reinhardt::installed_apps;

// installed_apps!マクロは以下を生成します:
// - 各アプリのバリアントを持つInstalledApp列挙型
// - 変換トレイトの実装（From、Into、Display）
// - アプリ設定と検出のためのレジストリ
//
// 注意: DjangoのINSTALLED_APPSとは異なり、このマクロはユーザーアプリのみ用です。
// 組み込みフレームワーク機能（auth、sessions、adminなど）は
// installed_apps!ではなくCargoの機能フラグで有効化します。
//
// 例:
// [dependencies]
// reinhardt = { version = "0.1", features = ["auth", "sessions", "admin"] }
//
// これにより以下が有効になります:
// - マイグレーション、管理パネルなどの自動アプリ検出
// - コード全体での型安全なアプリ参照
// - 一元化されたアプリ設定
installed_apps! {
	users: "users",
}

pub fn get_installed_apps() -> Vec<String> {
	InstalledApp::all_apps()
}
```

### 認証使用時

ReinhardtはDjangoスタイルの`BaseUser`と`FullUser`トレイトを持つユーザーモデルと、`UserManager`による包括的なユーザー管理を提供します。

**注意:** Reinhardtには組み込みの`DefaultUser`実装が含まれています。直接使用するか、以下に示すように独自のユーザーモデルを定義できます。

**ユーザー管理例:**

```rust
use reinhardt::prelude::*;

// UserManagerでユーザーを作成・管理
async fn manage_users() -> Result<(), Box<dyn std::error::Error>> {
	let hasher = Argon2Hasher::new();
	let user_manager = UserManager::new(hasher);

	// 新しいユーザーを作成
	let user = user_manager.create_user(CreateUserData {
		username: "alice".to_string(),
		email: "alice@example.com".to_string(),
		password: "secure_password".to_string(),
		first_name: Some("Alice".to_string()),
		last_name: Some("Smith".to_string()),
	}).await?;

	// ユーザー情報を更新
	user_manager.update_user(user.id, UpdateUserData {
		email: Some("alice.smith@example.com".to_string()),
		is_active: Some(true),
		..Default::default()
	}).await?;

	// グループと権限を管理
	let group_manager = GroupManager::new();
	let editors = group_manager.create_group(CreateGroupData {
		name: "editors".to_string(),
	}).await?;

	// オブジェクトレベルの権限を割り当て
	let permission = ObjectPermission::new("edit", user.id, article.id);
	let perm_checker = ObjectPermissionChecker::new();
	if perm_checker.has_permission(&user, "edit", &article).await? {
		// ユーザーは記事を編集可能
	}

	Ok(())
}
```

`users/models.rs`で組み込みの`DefaultUser`を使用:

```rust
// users/models.rs
use reinhardt::DefaultUser;

// DefaultUserをUserタイプとして再エクスポート
pub type User = DefaultUser;

// DefaultUserは以下を既に実装しています:
// - BaseUserトレイト（認証メソッド）
// - FullUserトレイト（username、email、first_name、last_nameなど）
// - PermissionsMixinトレイト（権限管理）
// - Modelトレイト（データベース操作）
```

**カスタムユーザーモデルの場合:**

DefaultUserを超える追加フィールドが必要な場合は、独自に定義:

```rust
// users/models.rs
use reinhardt::auth::{BaseUser, FullUser, PermissionsMixin};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

#[model(app_label = "users", table_name = "users")]
pub struct CustomUser {
	#[field(primary_key = true)]
	pub id: Uuid,

	#[field(max_length = 150)]
	pub username: String,

	#[field(max_length = 255)]
	pub email: String,

	pub password_hash: Option<String>,

	#[field(max_length = 150)]
	pub first_name: String,

	#[field(max_length = 150)]
	pub last_name: String,

	#[field(default = true)]
	pub is_active: bool,

	#[field(default = false)]
	pub is_staff: bool,

	#[field(default = false)]
	pub is_superuser: bool,

	pub last_login: Option<DateTime<Utc>>,

	#[field(auto_now_add = true)]
	pub date_joined: DateTime<Utc>,

	// カスタムフィールド
	#[field(max_length = 20, null = true)]
	pub phone_number: Option<String>,
}

impl BaseUser for CustomUser {
	type PrimaryKey = Uuid;

	fn get_username_field() -> &'static str { "username" }
	fn get_username(&self) -> &str { &self.username }
	fn password_hash(&self) -> Option<&str> { self.password_hash.as_deref() }
	fn set_password_hash(&mut self, hash: String) { self.password_hash = Some(hash); }
	fn last_login(&self) -> Option<DateTime<Utc>> { self.last_login }
	fn set_last_login(&mut self, time: DateTime<Utc>) { self.last_login = Some(time); }
	fn is_active(&self) -> bool { self.is_active }
}

impl FullUser for CustomUser {
	fn username(&self) -> &str { &self.username }
	fn email(&self) -> &str { &self.email }
	fn first_name(&self) -> &str { &self.first_name }
	fn last_name(&self) -> &str { &self.last_name }
	fn is_staff(&self) -> bool { self.is_staff }
	fn is_superuser(&self) -> bool { self.is_superuser }
	fn date_joined(&self) -> DateTime<Utc> { self.date_joined }
}
```

アプリの`views/profile.rs`でJWT認証を使用:

```rust
// users/views/profile.rs
use reinhardt::auth::{JwtAuth, BaseUser};
use reinhardt::{Request, Response, StatusCode, ViewResult, get};
use reinhardt::db::DatabaseConnection;
use std::sync::Arc;
use crate::models::User;

#[get("/profile", name = "get_profile")]
pub async fn get_profile(
	req: Request,
	#[inject] db: Arc<DatabaseConnection>,
) -> ViewResult<Response> {
	// AuthorizationヘッダーからJWTトークンを抽出
	let auth_header = req.headers.get("authorization")
		.and_then(|h| h.to_str().ok())
		.ok_or("Missing Authorization header")?;

	let token = auth_header.strip_prefix("Bearer ")
		.ok_or("Invalid Authorization header format")?;

	// トークンを検証してユーザーIDを取得
	let jwt_auth = JwtAuth::new(b"your-secret-key");
	let claims = jwt_auth.verify_token(token)?;

	// claims.user_idを使用してデータベースからユーザーを読み込み
	let user = User::find_by_id(&db, &claims.user_id).await?;

	// ユーザーがアクティブかチェック
	if !user.is_active() {
		return Err("User account is inactive".into());
	}

	// ユーザープロファイルをJSONとして返す
	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

### エンドポイント定義

ReinhardtはHTTPメソッドデコレーターを使用してエンドポイントを定義します:

#### HTTPメソッドデコレーター

`#[get]`、`#[post]`、`#[put]`、`#[delete]`を使用してルートを定義:

```rust
use reinhardt::{get, post, Request, Response, ViewResult};
use serde_json::json;

#[get("/")]
pub async fn hello(_req: Request) -> ViewResult<Response> {
	Ok(Response::ok().with_body("Hello, World!"))
}

#[post("/users")]
pub async fn create_user(_req: Request) -> ViewResult<Response> {
	let body = json!({"status": "created"});
	Response::ok().with_json(&body).map_err(Into::into)
}
```

**機能:**
- コンパイル時パス検証
- 簡潔な構文
- 自動HTTPメソッドバインディング
- `#[inject]`による依存性注入のサポート

#### 依存性注入の使用

HTTPメソッドデコレーターと`#[inject]`を組み合わせて自動依存性注入:

```rust
use reinhardt::{get, Request, Response, StatusCode, ViewResult};
use reinhardt::db::DatabaseConnection;
use std::sync::Arc;

#[get("/users/{id}/", name = "get_user")]
pub async fn get_user(
	req: Request,
	#[inject] db: Arc<DatabaseConnection>,  // 自動的に注入
) -> ViewResult<Response> {
	let id = req.path_params.get("id")
		.ok_or("Missing id")?
		.parse::<i64>()?;

	// 注入されたデータベース接続を使用
	let user = db.query("SELECT * FROM users WHERE id = $1")
		.bind(id)
		.fetch_one()
		.await?;

	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

**依存性注入の機能:**
- `#[inject]`属性による自動依存性注入
- `#[inject(cache = false)]`によるキャッシュ制御
- FastAPIにインスパイアされた依存性注入システム
- HTTPメソッドデコレーターとシームレスに連携

**戻り値の型:**

すべてのビュー関数は`ViewResult<T>`を戻り値の型として使用:

```rust
use reinhardt::ViewResult;  // 事前定義された結果型
```

### パラメータ抽出使用時

アプリの`views/user.rs`で:

```rust
// users/views/user.rs
use reinhardt::{Request, Response, StatusCode, ViewResult, get};
use reinhardt::db::DatabaseConnection;
use crate::models::User;
use std::sync::Arc;

#[get("/users/{id}/", name = "get_user")]
pub async fn get_user(
	req: Request,
	#[inject] db: Arc<DatabaseConnection>,
) -> ViewResult<Response> {
	// リクエストからパスパラメータを抽出
	let id = req.path_params.get("id")
		.ok_or("Missing id parameter")?
		.parse::<i64>()
		.map_err(|_| "Invalid id format")?;

	// クエリパラメータを抽出（例: ?include_inactive=true）
	let include_inactive = req.query_params.get("include_inactive")
		.and_then(|v| v.parse::<bool>().ok())
		.unwrap_or(false);

	// 注入された接続を使用してデータベースからユーザーを取得
	let user = User::find_by_id(&db, id).await?;

	// 必要に応じてアクティブステータスをチェック
	if !include_inactive && !user.is_active {
		return Err("User is inactive".into());
	}

	// JSONとして返す
	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

`urls.rs`でパスパラメータ付きルートを登録:

```rust
// users/urls.rs
use reinhardt::ServerRouter;

use super::views;

pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::get_user)  // パスは#[get("/users/{id}/")]で定義
}
```

### シリアライザーと検証使用時

アプリの`serializers/user.rs`で:

```rust
// users/serializers/user.rs
use serde::{Serialize, Deserialize};
use validator::Validate;

#[derive(Serialize, Deserialize, Validate)]
pub struct CreateUserRequest {
	#[validate(email)]
	pub email: String,
	#[validate(length(min = 3, max = 50))]
	pub username: String,
	#[validate(length(min = 8))]
	pub password: String,
}

#[derive(Serialize, Deserialize)]
pub struct UserResponse {
	pub id: i64,
	pub username: String,
	pub email: String,
	pub is_active: bool,
}

impl From<User> for UserResponse {
	fn from(user: User) -> Self {
		UserResponse {
			id: user.id,
			username: user.username,
			email: user.email,
			is_active: user.is_active,
		}
	}
}
```

アプリの`views/user.rs`で:

```rust
// users/views/user.rs
use reinhardt::{Request, Response, StatusCode, ViewResult, post};
use reinhardt::db::DatabaseConnection;
use crate::models::User;
use crate::serializers::{CreateUserRequest, UserResponse};
use validator::Validate;
use std::sync::Arc;

#[post("/users", name = "create_user")]
pub async fn create_user(
	mut req: Request,
	#[inject] db: Arc<DatabaseConnection>,
) -> ViewResult<Response> {
	// リクエストボディをパース
	let body_bytes = std::mem::take(&mut req.body);
	let create_req: CreateUserRequest = serde_json::from_slice(&body_bytes)?;

	// リクエストを検証
	create_req.validate()?;

	// ユーザーを作成
	let mut user = User {
		id: 0, // データベースによって設定される
		username: create_req.username,
		email: create_req.email,
		password_hash: None,
		is_active: true,
		created_at: Utc::now(),
	};

	// BaseUserトレイトを使用してパスワードをハッシュ化
	user.set_password(&create_req.password)?;

	// 注入された接続を使用してデータベースに保存
	user.save(&db).await?;

	// レスポンスに変換
	let response_data = UserResponse::from(user);
	let json = serde_json::to_string(&response_data)?;

	Ok(Response::new(StatusCode::CREATED)
		.with_body(json))
}
```

## 利用可能なコンポーネント

Reinhardtは組み合わせ可能なモジュラーコンポーネントを提供します:

| コンポーネント       | クレート名                 | 機能                                        |
|---------------------|---------------------------|---------------------------------------------|
| **コア**            |                           |                                             |
| コアタイプ          | `reinhardt-core`          | コアトレイト、型、マクロ（Model、endpoint） |
| HTTP＆ルーティング  | `reinhardt-http`          | Request/Response、HTTP処理                  |
| URLルーティング     | `reinhardt-urls`          | 関数ベースおよびクラスベースのルート        |
| サーバー            | `reinhardt-server`        | HTTPサーバー実装                            |
| ミドルウェア        | `reinhardt-dispatch`      | ミドルウェアチェーン、シグナルディスパッチ  |
| 設定                | `reinhardt-conf`          | 設定管理、環境読み込み                      |
| コマンド            | `reinhardt-commands`      | 管理CLIツール（startprojectなど）           |
| ショートカット      | `reinhardt-shortcuts`     | 一般的なユーティリティ関数                  |
| **データベース**    |                           |                                             |
| ORM                 | `reinhardt-db`            | reinhardt-query統合                         |
| **認証**            |                           |                                             |
| Auth                | `reinhardt-auth`          | JWT、Token、Session、Basic認証、Userモデル |
| **REST API**        |                           |                                             |
| シリアライザー      | `reinhardt-rest`          | serde/validator統合、ViewSets               |
| **フォーム**        |                           |                                             |
| フォーム            | `reinhardt-forms`         | フォーム処理と検証                          |
| **高度な機能**      |                           |                                             |
| 管理パネル          | `reinhardt-admin`         | Djangoスタイルの管理インターフェース        |
| プラグインシステム  | `reinhardt-dentdelion`    | 静的＆WASMプラグインサポート、CLI管理       |
| バックグラウンドタスク | `reinhardt-tasks`      | タスクキュー（Redis、RabbitMQ、SQLite）     |
| GraphQL             | `reinhardt-graphql`       | スキーマ生成、サブスクリプション            |
| WebSockets          | `reinhardt-websockets`    | リアルタイム通信                            |
| i18n                | `reinhardt-i18n`          | 多言語サポート                              |
| **テスト**          |                           |                                             |
| テストユーティリティ | `reinhardt-test`         | テストヘルパー、フィクスチャ、TestContainers |

**各クレート内の詳細な機能フラグについては、[機能フラグガイド](../FEATURE_FLAGS.md)を参照してください。**

---

## ドキュメント

- 📚 [はじめに](../GETTING_STARTED.md) - 初心者向けステップバイステップチュートリアル
- 🎛️ [機能フラグガイド](../FEATURE_FLAGS.md) - 詳細な機能制御でビルドを最適化
- 📖 [APIリファレンス](https://docs.rs/reinhardt)（近日公開）
- 📝 [チュートリアル](../tutorials/) - 実際のアプリケーションを構築して学ぶ

**AIアシスタント向け**: プロジェクト固有のコーディング標準、テストガイドライン、開発規約については[CLAUDE.md](../../CLAUDE.md)を参照してください。

## 💬 ヘルプを得る

Reinhardtはコミュニティ駆動のプロジェクトです。ヘルプが必要な場合:

- 💬 **Discord**: Discordサーバーでリアルタイムチャット（近日公開）
- 💭 **GitHub Discussions**: [質問やアイデアを共有](https://github.com/kent8192/reinhardt-web/discussions)
- 🐛 **Issues**: [バグを報告](https://github.com/kent8192/reinhardt-web/issues)
- 📖 **ドキュメント**: [ガイドを読む](../)

質問する前に、以下を確認してください:

- ✅ [はじめに](../GETTING_STARTED.md)
- ✅ [Examples](../../examples/)
- ✅ 既存のGitHub IssuesとDiscussions

## 🤝 コントリビューション

コントリビューションを歓迎します！始めるには[コントリビューティングガイド](../../CONTRIBUTING.md)をお読みください。

**クイックリンク**:

- [開発セットアップ](../../CONTRIBUTING.md#development-setup)
- [テストガイドライン](../../CONTRIBUTING.md#testing-guidelines)
- [コミットガイドライン](../../CONTRIBUTING.md#commit-guidelines)

## ⭐ スター履歴

<a href="https://star-history.com/#kent8192/reinhardt-web&Date">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date&theme=dark" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date" />
   <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date" width="600" />
 </picture>
</a>

## ライセンス

このプロジェクトは [BSD 3-Clause License](../../LICENSE) の下でライセンスされています。

### サードパーティ帰属

このプロジェクトは以下からインスピレーションを受けています:

- [Django](https://www.djangoproject.com/)（BSD 3-Clauseライセンス）
- [Django REST Framework](https://www.django-rest-framework.org/)（BSD 3-Clauseライセンス）
- [FastAPI](https://fastapi.tiangolo.com/)（MITライセンス）
- [SQLAlchemy](https://www.sqlalchemy.org/)（MITライセンス）

完全な帰属については[THIRD-PARTY-NOTICES](../../THIRD-PARTY-NOTICES)を参照してください。

**注意:** このプロジェクトはDjango Software Foundation、Encode OSS Ltd.、Sebastián Ramírez（FastAPI作者）、またはMichael Bayer（SQLAlchemy作者）と提携または承認されているわけではありません。
