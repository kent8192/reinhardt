<div align="center">
  <img src="../../branding/logo.png" alt="Reinhardt Logo" width="200"/>

  <h1>Reinhardt</h1>

  <h3>🦀 多石化電池內建</h3>

  <p><strong>Rust的可組合全端API框架</strong></p>
  <p>使用Django的「電池內建」哲學的<em>全部</em>力量構建，<br/>
  或只組合<em>你需要的</em>——你的選擇，你的方式。</p>

🌐 [English](../../README.md) | [日本語](README_JA.md) | [简体中文](README_ZH_CN.md) | **繁體中文** | [Русский](README_RU.md) | [Українська](README_UK.md) | [فارسی](README_FA.md) | [العربية](README_AR.md)

[![Crates.io](https://img.shields.io/crates/v/reinhardt-web.svg)](https://crates.io/crates/reinhardt-web)
[![Documentation](https://docs.rs/reinhardt-web/badge.svg)](https://docs.rs/reinhardt-web)
[![License](https://img.shields.io/badge/license-BSD--3--Clause-blue.svg)](../../LICENSE)
[![codecov](https://codecov.io/gh/kent8192/reinhardt-web/graph/badge.svg)](https://codecov.io/gh/kent8192/reinhardt-web)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/kent8192/reinhardt-web)

</div>

---

## 📍 快速導航

您可能在找:

- 🌐 [官方網站](https://reinhardt-web.dev) - 文件、教學和指南
- 🚀 [快速開始](#快速開始) - 5分鐘啟動運行
- 📦 [安裝選項](#安裝) - 選擇你的風格: Micro、Standard 或 Full
- 📚 [入門指南](https://reinhardt-web.dev/quickstart/getting-started/) - 分步教學
- 🎛️ [功能旗標](https://reinhardt-web.dev/docs/feature-flags/) - 微調你的構建
- 📖 [API文檔](https://docs.rs/reinhardt-web) - 完整API參考
- 💬 [社群與支援](#取得幫助) - 從社群獲取幫助

## 為什麼選擇Reinhardt?

**Polylithic = Poly（多）+ Lithic（構建塊）**
與強迫你使用所有功能的單體框架不同，Reinhardt讓你從獨立的、經過良好測試的元件中組合你的完美技術棧。

Reinhardt匯集了三個世界的精華:

| 靈感來源           | 我們借鑒了什麼                                         | 我們改進了什麼                                      |
|--------------------|--------------------------------------------------------|------------------------------------------------------|
| 🐍 **Django**      | 電池內建哲學、ORM設計、管理面板                         | 可組合構建的功能旗標、Rust的型別安全                  |
| 🎯 **Django REST** | 序列化器、ViewSets、權限                                | 編譯時驗證、零成本抽象                               |
| ⚡ **FastAPI**      | DI系統、自動OpenAPI                                     | 原生Rust效能、無執行時開銷                           |
| 🗄️ **SQLAlchemy** | QuerySet模式、關聯處理                                  | 型別安全的查詢建構器、編譯時驗證                      |

**結果**: 一個Python開發者熟悉的框架，但擁有Rust的效能和安全保證。

## ✨ 主要功能

- **型別安全的ORM** 帶編譯時驗證（reinhardt-query）
- **強大的序列化器** 帶自動驗證（serde + validator）
- **FastAPI風格的DI** 帶型別安全的依賴注入和快取
- **ViewSets** 用於快速CRUD API開發
- **多重認證**（JWT、Token、Session、Basic）帶BaseUser/FullUser traits
- **管理面板** 自動生成的模型管理介面
- **管理命令** 用於遷移、靜態檔案等
- **GraphQL和WebSocket** 支援即時應用
- **分頁、過濾、速率限制** 內建
- **訊號** 用於事件驅動架構

完整列表請參閱[可用元件](#可用元件)，範例請參閱[入門指南](https://reinhardt-web.dev/quickstart/getting-started/)。

## 安裝

Reinhardt是一個模組化框架。選擇你的起點:

**關於Crate命名的說明:**
主Reinhardt crate在crates.io上發布為`reinhardt-web`，但你在程式碼中使用`package`屬性將其匯入為`reinhardt`。

### 預設: 全功能（電池內建）⚠️ 新預設

零配置獲取所有功能:

```toml
[dependencies]
# 匯入為'reinhardt'，發布為'reinhardt-web'
# 預設啟用所有功能（完整套裝）
reinhardt = { version = "0.1.0-rc.1", package = "reinhardt-web" }
```

**包含:** Database、Auth、REST API、Admin、GraphQL、WebSockets、Cache、i18n、Mail、Sessions、Static Files、Storage

**二進位大小**: ~50+ MB | **編譯**: 較慢，但一切開箱即用

然後在程式碼中使用:
```rust
use reinhardt::prelude::*;
use reinhardt::{Request, Response, StatusCode};
```

### 選項1: 標準設定（平衡型）

適用於不需要所有功能的大多數專案:

```toml
[dependencies]
reinhardt = { version = "0.1.0-rc.1", package = "reinhardt-web", default-features = false, features = ["standard"] }
```

**包含:** Core、Database（PostgreSQL）、REST API、Auth、Middleware、Pages（帶SSR的WASM前端）

**二進位大小**: ~20-30 MB | **編譯**: 中等

### 選項2: 微服務（最小設定）

輕量快速，適合簡單API:

```toml
[dependencies]
reinhardt = { version = "0.1.0-rc.1", package = "reinhardt-web", default-features = false, features = ["minimal"] }
```

**包含:** HTTP、路由、DI、參數提取、伺服器

**二進位大小**: ~5-10 MB | **編譯**: 非常快

### 選項3: 構建你的自訂技術棧

只安裝你需要的元件:

```toml
[dependencies]
# 核心元件
reinhardt-http = "0.1.0-rc.1"
reinhardt-urls = "0.1.0-rc.1"

# 可選: 資料庫
reinhardt-db = "0.1.0-rc.1"

# 可選: 認證
reinhardt-auth = "0.1.0-rc.1"

# 可選: REST API功能
reinhardt-rest = "0.1.0-rc.1"

# 可選: 管理面板
reinhardt-admin = "0.1.0-rc.1"

# 可選: 進階功能
reinhardt-graphql = "0.1.0-rc.1"
reinhardt-websockets = "0.1.0-rc.1"
```

**📖 完整的可用crates和功能旗標列表，請參閱[功能旗標指南](https://reinhardt-web.dev/docs/feature-flags/)。**

## 快速開始

### 1. 安裝Reinhardt管理工具

```bash
cargo install reinhardt-admin-cli
```

### 2. 建立新專案

```bash
# 建立RESTful API專案（預設）
reinhardt-admin startproject my-api
cd my-api
```

這將生成完整的專案結構:

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

**備選方案: 建立reinhardt-pages專案（WASM + SSR）**

用於帶SSR的現代WASM前端:

```bash
# 建立pages專案
reinhardt-admin startproject my-app --with-pages
cd my-app

# 安裝WASM構建工具（僅首次）
cargo make install-wasm-tools

# 構建WASM並啟動開發伺服器
cargo make dev
# 訪問 http://127.0.0.1:8000/
```

### 3. 執行開發伺服器

```bash
# 使用manage命令
cargo run --bin manage runserver

# 伺服器將在 http://127.0.0.1:8000 啟動
```

**自動重載支援:**

程式碼變更時自動重載（需要bacon）:

```bash
# 安裝bacon
cargo install --locked bacon

# 帶自動重載執行
bacon runserver

# 或使用cargo make
cargo make watch

# 用於測試
bacon test
```

### 4. 建立你的第一個App

```bash
# 建立RESTful API app（預設）
cargo run --bin manage startapp users

# 或明確指定類型
cargo run --bin manage startapp users --restful

# 建立Pages app（WASM + SSR）
cargo run --bin manage startapp dashboard --with-pages
```

這將建立app結構:

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

### 5. 註冊路由

編輯你的app的`urls.rs`:

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

在`src/config/urls.rs`中包含:

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

`#[routes]`屬性巨集透過`inventory` crate自動向框架註冊此函數以供發現。

**注意:** `reinhardt::prelude`包含常用型別。主要匯出包括:

**始終可用:**
- 核心路由和視圖: `Router`、`DefaultRouter`、`ServerRouter`、`View`、`ListView`、`DetailView`
- ViewSets: `ViewSet`、`ModelViewSet`、`ReadOnlyModelViewSet`
- HTTP: `StatusCode`

**功能依賴:**
- **`core`功能**: `Request`、`Response`、`Handler`、`Middleware`、訊號（`post_save`、`pre_save`等）
- **`database`功能**: `Model`、`DatabaseConnection`、`F`、`Q`、`Transaction`、`atomic`、資料庫函數（`Concat`、`Upper`、`Lower`、`Now`、`CurrentDate`）、視窗函數（`Window`、`RowNumber`、`Rank`、`DenseRank`）、約束（`UniqueConstraint`、`CheckConstraint`、`ForeignKeyConstraint`）
- **`auth`功能**: `User`、`UserManager`、`GroupManager`、`Permission`、`ObjectPermission`
- **`minimal`、`standard`或`di`功能**: `Body`、`Cookie`、`Header`、`Json`、`Path`、`Query`
- **`rest`功能**: 序列化器、解析器、分頁、限流、版本控制
- **`admin`功能**: 管理面板元件
- **`cache`功能**: `Cache`、`InMemoryCache`
- **`sessions`功能**: `Session`、`AuthenticationMiddleware`

完整列表請參閱[功能旗標指南](https://reinhardt-web.dev/docs/feature-flags/)。

完整的分步指南請參閱[入門指南](https://reinhardt-web.dev/quickstart/getting-started/)。

## 🎓 透過範例學習

### 使用資料庫

在`settings/base.toml`中配置資料庫:

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

設定在`src/config/settings.rs`中自動載入:

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

**環境變數來源:**

Reinhardt提供兩種具有不同優先級的環境變數來源:

- **`EnvSource`**（優先級: 100）- 覆蓋TOML檔案的高優先級環境變數
  ```rust
  .add_source(EnvSource::new().with_prefix("REINHARDT_"))
  ```

- **`LowPriorityEnvSource`**（優先級: 40）- 回退到TOML檔案的低優先級環境變數
  ```rust
  .add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
  ```

**優先級順序**:
- 使用`EnvSource`: 環境變數 > `{profile}.toml` > `base.toml` > 預設值
- 使用`LowPriorityEnvSource`（如上所示）: `{profile}.toml` > `base.toml` > 環境變數 > 預設值

當環境變數應始終優先時選擇`EnvSource`（例如生產部署）。
當TOML檔案應為主要配置來源時選擇`LowPriorityEnvSource`（例如開發）。

詳情請參閱[設定文檔](docs/SETTINGS_DOCUMENT.md)。

**使用內建DefaultUser:**

Reinhardt提供即用型`DefaultUser`實作（需要`argon2-hasher`功能）:

```rust
// users/models.rs
use reinhardt::prelude::*;
use reinhardt::DefaultUser;

// 將DefaultUser重新匯出為你的app的User
pub type User = DefaultUser;

// DefaultUser包含:
// - id: Uuid（主鍵）
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

// DefaultUser實作:
// - BaseUser trait（認證方法）
// - FullUser trait（完整使用者資訊）
// - PermissionsMixin trait（權限管理）
// - Model trait（資料庫操作）
```

**定義自訂使用者模型:**

如果需要自訂欄位，定義你自己的模型:

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

	// 添加自訂欄位
	#[field(max_length = 50, null = true)]
	pub phone_number: Option<String>,
}
```

**Model屬性巨集:**

`#[model(...)]`屬性自動生成:
- `Model` trait的實作（包含`#[derive(Model)]`功能）
- 型別安全的欄位存取器: `User::field_email()`、`User::field_username()`等
- 全域模型註冊表註冊
- 複合主鍵支援

**注意:** 使用`#[model(...)]`時，不需要單獨添加`#[derive(Model)]`，
它由`#[model(...)]`屬性自動應用。

**欄位屬性:**
- `#[field(primary_key = true)]` - 標記為主鍵
- `#[field(max_length = 255)]` - 設定字串欄位的最大長度
- `#[field(default = value)]` - 設定預設值
- `#[field(auto_now_add = true)]` - 建立時自動填充時間戳
- `#[field(auto_now = true)]` - 儲存時自動更新時間戳
- `#[field(null = true)]` - 允許NULL值
- `#[field(unique = true)]` - 強制唯一性約束

完整的欄位屬性列表請參閱[欄位屬性指南](docs/field_attributes.md)。

生成的欄位存取器在查詢中啟用型別安全的欄位引用:

```rust
// 由#[model(...)]為DefaultUser生成
impl DefaultUser {
	pub const fn field_id() -> FieldRef<DefaultUser, Uuid> { FieldRef::new("id") }
	pub const fn field_username() -> FieldRef<DefaultUser, String> { FieldRef::new("username") }
	pub const fn field_email() -> FieldRef<DefaultUser, String> { FieldRef::new("email") }
	pub const fn field_is_active() -> FieldRef<DefaultUser, bool> { FieldRef::new("is_active") }
	pub const fn field_is_staff() -> FieldRef<DefaultUser, bool> { FieldRef::new("is_staff") }
	pub const fn field_date_joined() -> FieldRef<DefaultUser, DateTime<Utc>> { FieldRef::new("date_joined") }
	// ... 其他欄位
}
```

**進階查詢範例:**

```rust
use reinhardt::prelude::*;
use reinhardt::DefaultUser;

// 使用型別安全欄位引用的Django風格F/Q物件查詢
async fn complex_user_query() -> Result<Vec<DefaultUser>, Box<dyn std::error::Error>> {
	// 使用型別安全欄位引用的Q物件（使用生成的欄位存取器）
	let active_query = Q::new()
		.field("is_active").eq(true)
		.and(Q::new().field("date_joined").gte(Now::new()));

	// 使用型別安全欄位引用的資料庫函數
	let email_lower = Lower::new(DefaultUser::field_email().into());
	let username_upper = Upper::new(DefaultUser::field_username().into());

	// 使用欄位存取器的聚合
	let user_count = Aggregate::count(DefaultUser::field_id().into());
	let latest_joined = Aggregate::max(DefaultUser::field_date_joined().into());

	// 用於排名的視窗函數
	let rank_by_join_date = Window::new()
		.partition_by(vec![DefaultUser::field_is_active().into()])
		.order_by(vec![(DefaultUser::field_date_joined().into(), "DESC")])
		.function(RowNumber::new());

	todo!("Execute query with these components")
}

// 交易支援
async fn create_user_with_transaction(
	conn: &DatabaseConnection,
	user_data: CreateUserRequest
) -> Result<User, Box<dyn std::error::Error>> {
	// 出錯時自動回滾的交易
	transaction(conn, |_tx| async move {
		let user = User::create(user_data).await?;
		log_user_creation(&user).await?;
		Ok(user)
	}).await
}
```

**注意**: Reinhardt使用reinhardt-query進行SQL操作。`#[derive(Model)]`巨集自動生成Model trait實作、型別安全的欄位存取器和全域模型註冊表註冊。

在`src/config/apps.rs`中註冊:

```rust
// src/config/apps.rs
use reinhardt::installed_apps;

// installed_apps!巨集生成:
// - 每個app變體的InstalledApp列舉
// - 轉換trait的實作（From、Into、Display）
// - app配置和發現的註冊表
//
// 注意: 與Django的INSTALLED_APPS不同，此巨集僅用於使用者apps。
// 內建框架功能（auth、sessions、admin等）透過
// Cargo功能旗標啟用，而不是透過installed_apps!。
//
// 範例:
// [dependencies]
// reinhardt = { version = "0.1", features = ["auth", "sessions", "admin"] }
//
// 這啟用:
// - 遷移、管理面板等的自動app發現
// - 程式碼中的型別安全app引用
// - 集中的app配置
installed_apps! {
	users: "users",
}

pub fn get_installed_apps() -> Vec<String> {
	InstalledApp::all_apps()
}
```

### 使用認證

Reinhardt提供Django風格的使用者模型，帶有`BaseUser`和`FullUser` traits，以及透過`UserManager`的全面使用者管理。

**注意:** Reinhardt包含內建的`DefaultUser`實作。你可以直接使用它或如下所示定義自己的使用者模型。

**使用者管理範例:**

```rust
use reinhardt::prelude::*;

// 使用UserManager建立和管理使用者
async fn manage_users() -> Result<(), Box<dyn std::error::Error>> {
	let hasher = Argon2Hasher::new();
	let user_manager = UserManager::new(hasher);

	// 建立新使用者
	let user = user_manager.create_user(CreateUserData {
		username: "alice".to_string(),
		email: "alice@example.com".to_string(),
		password: "secure_password".to_string(),
		first_name: Some("Alice".to_string()),
		last_name: Some("Smith".to_string()),
	}).await?;

	// 更新使用者資訊
	user_manager.update_user(user.id, UpdateUserData {
		email: Some("alice.smith@example.com".to_string()),
		is_active: Some(true),
		..Default::default()
	}).await?;

	// 管理群組和權限
	let group_manager = GroupManager::new();
	let editors = group_manager.create_group(CreateGroupData {
		name: "editors".to_string(),
	}).await?;

	// 分配物件級權限
	let permission = ObjectPermission::new("edit", user.id, article.id);
	let perm_checker = ObjectPermissionChecker::new();
	if perm_checker.has_permission(&user, "edit", &article).await? {
		// 使用者可以編輯文章
	}

	Ok(())
}
```

在`users/models.rs`中使用內建的`DefaultUser`:

```rust
// users/models.rs
use reinhardt::DefaultUser;

// 將DefaultUser重新匯出為你的User型別
pub type User = DefaultUser;

// DefaultUser已實作:
// - BaseUser trait（認證方法）
// - FullUser trait（username、email、first_name、last_name等）
// - PermissionsMixin trait（權限管理）
// - Model trait（資料庫操作）
```

**對於自訂使用者模型:**

如果需要超出DefaultUser的額外欄位，定義你自己的:

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

	// 自訂欄位
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

在app的`views/profile.rs`中使用JWT認證:

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
	// 從Authorization標頭提取JWT令牌
	let auth_header = req.headers.get("authorization")
		.and_then(|h| h.to_str().ok())
		.ok_or("Missing Authorization header")?;

	let token = auth_header.strip_prefix("Bearer ")
		.ok_or("Invalid Authorization header format")?;

	// 驗證令牌並獲取使用者ID
	let jwt_auth = JwtAuth::new(b"your-secret-key");
	let claims = jwt_auth.verify_token(token)?;

	// 使用claims.user_id從資料庫載入使用者
	let user = User::find_by_id(&db, &claims.user_id).await?;

	// 檢查使用者是否活躍
	if !user.is_active() {
		return Err("User account is inactive".into());
	}

	// 返回使用者設定檔為JSON
	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

### 端點定義

Reinhardt使用HTTP方法裝飾器定義端點:

#### HTTP方法裝飾器

使用`#[get]`、`#[post]`、`#[put]`、`#[delete]`定義路由:

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

**功能:**
- 編譯時路徑驗證
- 簡潔語法
- 自動HTTP方法綁定
- 透過`#[inject]`支援依賴注入

#### 使用依賴注入

將HTTP方法裝飾器與`#[inject]`結合進行自動依賴注入:

```rust
use reinhardt::{get, Request, Response, StatusCode, ViewResult};
use reinhardt::db::DatabaseConnection;
use std::sync::Arc;

#[get("/users/{id}/", name = "get_user")]
pub async fn get_user(
	req: Request,
	#[inject] db: Arc<DatabaseConnection>,  // 自動注入
) -> ViewResult<Response> {
	let id = req.path_params.get("id")
		.ok_or("Missing id")?
		.parse::<i64>()?;

	// 使用注入的資料庫連線
	let user = db.query("SELECT * FROM users WHERE id = $1")
		.bind(id)
		.fetch_one()
		.await?;

	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

**依賴注入功能:**
- 透過`#[inject]`屬性自動依賴注入
- 透過`#[inject(cache = false)]`控制快取
- FastAPI啟發的依賴注入系統
- 與HTTP方法裝飾器無縫協作

**回傳型別:**

所有視圖函數使用`ViewResult<T>`作為回傳型別:

```rust
use reinhardt::ViewResult;  // 預定義結果型別
```

### 使用參數提取

在app的`views/user.rs`中:

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
	// 從請求提取路徑參數
	let id = req.path_params.get("id")
		.ok_or("Missing id parameter")?
		.parse::<i64>()
		.map_err(|_| "Invalid id format")?;

	// 提取查詢參數（例如 ?include_inactive=true）
	let include_inactive = req.query_params.get("include_inactive")
		.and_then(|v| v.parse::<bool>().ok())
		.unwrap_or(false);

	// 使用注入的連線從資料庫獲取使用者
	let user = User::find_by_id(&db, id).await?;

	// 如需檢查活躍狀態
	if !include_inactive && !user.is_active {
		return Err("User is inactive".into());
	}

	// 回傳JSON
	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

在`urls.rs`中註冊帶路徑參數的路由:

```rust
// users/urls.rs
use reinhardt::ServerRouter;

use super::views;

pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::get_user)  // 路徑在#[get("/users/{id}/")]中定義
}
```

### 使用序列化器和驗證

在app的`serializers/user.rs`中:

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

在app的`views/user.rs`中:

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
	// 解析請求本體
	let body_bytes = std::mem::take(&mut req.body);
	let create_req: CreateUserRequest = serde_json::from_slice(&body_bytes)?;

	// 驗證請求
	create_req.validate()?;

	// 建立使用者
	let mut user = User {
		id: 0, // 將由資料庫設定
		username: create_req.username,
		email: create_req.email,
		password_hash: None,
		is_active: true,
		created_at: Utc::now(),
	};

	// 使用BaseUser trait雜湊密碼
	user.set_password(&create_req.password)?;

	// 使用注入的連線儲存到資料庫
	user.save(&db).await?;

	// 轉換為回應
	let response_data = UserResponse::from(user);
	let json = serde_json::to_string(&response_data)?;

	Ok(Response::new(StatusCode::CREATED)
		.with_body(json))
}
```

## 可用元件

Reinhardt提供可混合搭配的模組化元件:

| 元件               | Crate名稱                  | 功能                                        |
|---------------------|---------------------------|---------------------------------------------|
| **核心**            |                           |                                             |
| 核心型別            | `reinhardt-core`          | 核心traits、型別、巨集（Model、endpoint）   |
| HTTP和路由          | `reinhardt-http`          | Request/Response、HTTP處理                  |
| URL路由             | `reinhardt-urls`          | 函數式和類別式路由                          |
| 伺服器              | `reinhardt-server`        | HTTP伺服器實作                              |
| 中介軟體            | `reinhardt-dispatch`      | 中介軟體鏈、訊號分發                        |
| 配置                | `reinhardt-conf`          | 設定管理、環境載入                          |
| 命令                | `reinhardt-commands`      | 管理CLI工具（startproject等）               |
| 捷徑                | `reinhardt-shortcuts`     | 常用工具函數                                |
| **資料庫**          |                           |                                             |
| ORM                 | `reinhardt-db`            | reinhardt-query整合                         |
| **認證**            |                           |                                             |
| Auth                | `reinhardt-auth`          | JWT、Token、Session、Basic認證、使用者模型 |
| **REST API**        |                           |                                             |
| 序列化器            | `reinhardt-rest`          | serde/validator整合、ViewSets               |
| **表單**            |                           |                                             |
| 表單                | `reinhardt-forms`         | 表單處理和驗證                              |
| **進階功能**        |                           |                                             |
| 管理面板            | `reinhardt-admin`         | Django風格管理介面                          |
| 外掛系統            | `reinhardt-dentdelion`    | 靜態和WASM外掛支援、CLI管理                 |
| 背景任務            | `reinhardt-tasks`         | 任務佇列（Redis、RabbitMQ、SQLite）         |
| GraphQL             | `reinhardt-graphql`       | Schema生成、訂閱                            |
| WebSockets          | `reinhardt-websockets`    | 即時通訊                                    |
| i18n                | `reinhardt-i18n`          | 多語言支援                                  |
| **測試**            |                           |                                             |
| 測試工具            | `reinhardt-test`          | 測試輔助、fixtures、TestContainers          |

**各crate內的詳細功能旗標，請參閱[功能旗標指南](https://reinhardt-web.dev/docs/feature-flags/)。**

---

## 文檔

- 📚 [入門指南](https://reinhardt-web.dev/quickstart/getting-started/) - 初學者分步教學
- 🎛️ [功能旗標指南](https://reinhardt-web.dev/docs/feature-flags/) - 透過細粒度功能控制最佳化構建
- 📖 [API參考](https://docs.rs/reinhardt)（即將推出）
- 📝 [教學](https://reinhardt-web.dev/quickstart/tutorials/) - 透過構建真實應用學習

**AI助手請參閱**: 專案特定的編碼標準、測試指南和開發慣例請參閱[CLAUDE.md](CLAUDE.md)。

## 💬 取得幫助

Reinhardt是一個社群驅動的專案。以下是取得幫助的途徑:

- 💬 **Discord**: 加入我們的Discord伺服器進行即時聊天（即將推出）
- 💭 **GitHub Discussions**: [提問和分享想法](https://github.com/kent8192/reinhardt-web/discussions)
- 🐛 **Issues**: [報告bug](https://github.com/kent8192/reinhardt-web/issues)
- 📖 **文檔**: [閱讀指南](../)

提問前，請查看:

- ✅ [入門指南](https://reinhardt-web.dev/quickstart/getting-started/)
- ✅ [Examples](../../examples/)
- ✅ 現有的GitHub Issues和Discussions

## 🤝 貢獻

我們歡迎貢獻！請閱讀[貢獻指南](../../CONTRIBUTING.md)開始。

**快速連結**:

- [開發設定](../../CONTRIBUTING.md#development-setup)
- [測試指南](../../CONTRIBUTING.md#testing-guidelines)
- [提交指南](../../CONTRIBUTING.md#commit-guidelines)

## ⭐ Star 趨勢

<a href="https://star-history.com/#kent8192/reinhardt-web&Date">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date&theme=dark" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date" />
   <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date" width="600" />
 </picture>
</a>

## 授權

本專案基於 [BSD 3-Clause License](../../LICENSE) 授權。

### 第三方歸屬

本專案受以下專案啟發:

- [Django](https://www.djangoproject.com/)（BSD 3-Clause授權）
- [Django REST Framework](https://www.django-rest-framework.org/)（BSD 3-Clause授權）
- [FastAPI](https://fastapi.tiangolo.com/)（MIT授權）
- [SQLAlchemy](https://www.sqlalchemy.org/)（MIT授權）

完整歸屬請參閱[THIRD-PARTY-NOTICES](../../THIRD-PARTY-NOTICES)。

**注意:** 本專案不隸屬於Django Software Foundation、Encode OSS Ltd.、Sebastián Ramírez（FastAPI作者）或Michael Bayer（SQLAlchemy作者），也未獲得其認可。
