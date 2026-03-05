<div align="center">
  <img src="../../branding/logo.png" alt="Reinhardt Logo" width="200"/>

  <h1>Reinhardt</h1>

  <h3>🦀 多石化电池内置</h3>

  <p><strong>Rust的可组合全栈API框架</strong></p>
  <p>使用Django的"电池内置"哲学的<em>全部</em>力量构建，<br/>
  或只组合<em>你需要的</em>——你的选择，你的方式。</p>

🌐 [English](../../README.md) | [日本語](README_JA.md) | **简体中文** | [繁體中文](README_ZH_TW.md) | [Русский](README_RU.md) | [Українська](README_UK.md) | [فارسی](README_FA.md) | [العربية](README_AR.md)

[![Crates.io](https://img.shields.io/crates/v/reinhardt-web.svg)](https://crates.io/crates/reinhardt-web)
[![Documentation](https://docs.rs/reinhardt-web/badge.svg)](https://docs.rs/reinhardt-web)
[![License](https://img.shields.io/badge/license-BSD--3--Clause-blue.svg)](../../LICENSE)
[![codecov](https://codecov.io/gh/kent8192/reinhardt-web/graph/badge.svg)](https://codecov.io/gh/kent8192/reinhardt-web)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/kent8192/reinhardt-web)

</div>

---

## 📍 快速导航

您可能在找:

- 🌐 [官方网站](https://reinhardt-web.dev) - 文档、教程和指南
- 🚀 [快速开始](#快速开始) - 5分钟启动运行
- 📦 [安装选项](#安装) - 选择你的风格: Micro、Standard 或 Full
- 📚 [入门指南](https://reinhardt-web.dev/quickstart/getting-started/) - 分步教程
- 🎛️ [功能标志](https://reinhardt-web.dev/docs/feature-flags/) - 微调你的构建
- 📖 [API文档](https://docs.rs/reinhardt-web) - 完整API参考
- 💬 [社区与支持](#获取帮助) - 从社区获取帮助

## 为什么选择Reinhardt?

**Polylithic = Poly（多）+ Lithic（构建块）**
与强迫你使用所有功能的单体框架不同，Reinhardt让你从独立的、经过良好测试的组件中组合你的完美技术栈。

Reinhardt汇集了三个世界的精华:

| 灵感来源           | 我们借鉴了什么                                         | 我们改进了什么                                      |
|--------------------|--------------------------------------------------------|------------------------------------------------------|
| 🐍 **Django**      | 电池内置哲学、ORM设计、管理面板                         | 可组合构建的功能标志、Rust的类型安全                  |
| 🎯 **Django REST** | 序列化器、ViewSets、权限                                | 编译时验证、零成本抽象                               |
| ⚡ **FastAPI**      | DI系统、自动OpenAPI                                     | 原生Rust性能、无运行时开销                           |
| 🗄️ **SQLAlchemy** | QuerySet模式、关系处理                                  | 类型安全的查询构建器、编译时验证                      |

**结果**: 一个Python开发者熟悉的框架，但拥有Rust的性能和安全保证。

## ✨ 主要功能

- **类型安全的ORM** 带编译时验证（reinhardt-query）
- **强大的序列化器** 带自动验证（serde + validator）
- **FastAPI风格的DI** 带类型安全的依赖注入和缓存
- **ViewSets** 用于快速CRUD API开发
- **多重认证**（JWT、Token、Session、Basic）带BaseUser/FullUser traits
- **管理面板** 自动生成的模型管理界面
- **管理命令** 用于迁移、静态文件等
- **GraphQL和WebSocket** 支持实时应用
- **分页、过滤、速率限制** 内置
- **信号** 用于事件驱动架构

完整列表请参阅[可用组件](#可用组件)，示例请参阅[入门指南](https://reinhardt-web.dev/quickstart/getting-started/)。

## 安装

Reinhardt是一个模块化框架。选择你的起点:

**关于Crate命名的说明:**
主Reinhardt crate在crates.io上发布为`reinhardt-web`，但你在代码中使用`package`属性将其导入为`reinhardt`。

### 默认: 全功能（电池内置）⚠️ 新默认

零配置获取所有功能:

```toml
[dependencies]
# 导入为'reinhardt'，发布为'reinhardt-web'
# 默认启用所有功能（完整捆绑包）
reinhardt = { version = "0.1.0-rc.1", package = "reinhardt-web" }
```

**包含:** Database、Auth、REST API、Admin、GraphQL、WebSockets、Cache、i18n、Mail、Sessions、Static Files、Storage

**二进制大小**: ~50+ MB | **编译**: 较慢，但一切开箱即用

然后在代码中使用:
```rust
use reinhardt::prelude::*;
use reinhardt::{Request, Response, StatusCode};
```

### 选项1: 标准设置（平衡型）

适用于不需要所有功能的大多数项目:

```toml
[dependencies]
reinhardt = { version = "0.1.0-rc.1", package = "reinhardt-web", default-features = false, features = ["standard"] }
```

**包含:** Core、Database（PostgreSQL）、REST API、Auth、Middleware、Pages（带SSR的WASM前端）

**二进制大小**: ~20-30 MB | **编译**: 中等

### 选项2: 微服务（最小设置）

轻量快速，适合简单API:

```toml
[dependencies]
reinhardt = { version = "0.1.0-rc.1", package = "reinhardt-web", default-features = false, features = ["minimal"] }
```

**包含:** HTTP、路由、DI、参数提取、服务器

**二进制大小**: ~5-10 MB | **编译**: 非常快

### 选项3: 构建你的自定义技术栈

只安装你需要的组件:

```toml
[dependencies]
# 核心组件
reinhardt-http = "0.1.0-rc.1"
reinhardt-urls = "0.1.0-rc.1"

# 可选: 数据库
reinhardt-db = "0.1.0-rc.1"

# 可选: 认证
reinhardt-auth = "0.1.0-rc.1"

# 可选: REST API功能
reinhardt-rest = "0.1.0-rc.1"

# 可选: 管理面板
reinhardt-admin = "0.1.0-rc.1"

# 可选: 高级功能
reinhardt-graphql = "0.1.0-rc.1"
reinhardt-websockets = "0.1.0-rc.1"
```

**📖 完整的可用crates和功能标志列表，请参阅[功能标志指南](https://reinhardt-web.dev/docs/feature-flags/)。**

## 快速开始

### 1. 安装Reinhardt管理工具

```bash
cargo install reinhardt-admin-cli
```

### 2. 创建新项目

```bash
# 创建RESTful API项目（默认）
reinhardt-admin startproject my-api
cd my-api
```

这将生成完整的项目结构:

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

**备选方案: 创建reinhardt-pages项目（WASM + SSR）**

用于带SSR的现代WASM前端:

```bash
# 创建pages项目
reinhardt-admin startproject my-app --with-pages
cd my-app

# 安装WASM构建工具（仅首次）
cargo make install-wasm-tools

# 构建WASM并启动开发服务器
cargo make dev
# 访问 http://127.0.0.1:8000/
```

### 3. 运行开发服务器

```bash
# 使用manage命令
cargo run --bin manage runserver

# 服务器将在 http://127.0.0.1:8000 启动
```

**自动重载支持:**

代码更改时自动重载（需要bacon）:

```bash
# 安装bacon
cargo install --locked bacon

# 带自动重载运行
bacon runserver

# 或使用cargo make
cargo make watch

# 用于测试
bacon test
```

### 4. 创建你的第一个App

```bash
# 创建RESTful API app（默认）
cargo run --bin manage startapp users

# 或明确指定类型
cargo run --bin manage startapp users --restful

# 创建Pages app（WASM + SSR）
cargo run --bin manage startapp dashboard --with-pages
```

这将创建app结构:

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

### 5. 注册路由

编辑你的app的`urls.rs`:

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

`#[routes]`属性宏通过`inventory` crate自动向框架注册此函数以供发现。

**注意:** `reinhardt::prelude`包含常用类型。主要导出包括:

**始终可用:**
- 核心路由和视图: `Router`、`DefaultRouter`、`ServerRouter`、`View`、`ListView`、`DetailView`
- ViewSets: `ViewSet`、`ModelViewSet`、`ReadOnlyModelViewSet`
- HTTP: `StatusCode`

**功能依赖:**
- **`core`功能**: `Request`、`Response`、`Handler`、`Middleware`、信号（`post_save`、`pre_save`等）
- **`database`功能**: `Model`、`DatabaseConnection`、`F`、`Q`、`Transaction`、`atomic`、数据库函数（`Concat`、`Upper`、`Lower`、`Now`、`CurrentDate`）、窗口函数（`Window`、`RowNumber`、`Rank`、`DenseRank`）、约束（`UniqueConstraint`、`CheckConstraint`、`ForeignKeyConstraint`）
- **`auth`功能**: `User`、`UserManager`、`GroupManager`、`Permission`、`ObjectPermission`
- **`minimal`、`standard`或`di`功能**: `Body`、`Cookie`、`Header`、`Json`、`Path`、`Query`
- **`rest`功能**: 序列化器、解析器、分页、限流、版本控制
- **`admin`功能**: 管理面板组件
- **`cache`功能**: `Cache`、`InMemoryCache`
- **`sessions`功能**: `Session`、`AuthenticationMiddleware`

完整列表请参阅[功能标志指南](https://reinhardt-web.dev/docs/feature-flags/)。

完整的分步指南请参阅[入门指南](https://reinhardt-web.dev/quickstart/getting-started/)。

## 🎓 通过示例学习

### 使用数据库

在`settings/base.toml`中配置数据库:

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

设置在`src/config/settings.rs`中自动加载:

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

**环境变量源:**

Reinhardt提供两种具有不同优先级的环境变量源:

- **`EnvSource`**（优先级: 100）- 覆盖TOML文件的高优先级环境变量
  ```rust
  .add_source(EnvSource::new().with_prefix("REINHARDT_"))
  ```

- **`LowPriorityEnvSource`**（优先级: 40）- 回退到TOML文件的低优先级环境变量
  ```rust
  .add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
  ```

**优先级顺序**:
- 使用`EnvSource`: 环境变量 > `{profile}.toml` > `base.toml` > 默认值
- 使用`LowPriorityEnvSource`（如上所示）: `{profile}.toml` > `base.toml` > 环境变量 > 默认值

当环境变量应始终优先时选择`EnvSource`（例如生产部署）。
当TOML文件应为主要配置源时选择`LowPriorityEnvSource`（例如开发）。

详情请参阅[设置文档](docs/SETTINGS_DOCUMENT.md)。

**使用内置DefaultUser:**

Reinhardt提供即用型`DefaultUser`实现（需要`argon2-hasher`功能）:

```rust
// users/models.rs
use reinhardt::prelude::*;
use reinhardt::DefaultUser;

// 将DefaultUser重新导出为你的app的User
pub type User = DefaultUser;

// DefaultUser包含:
// - id: Uuid（主键）
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

// DefaultUser实现:
// - BaseUser trait（认证方法）
// - FullUser trait（完整用户信息）
// - PermissionsMixin trait（权限管理）
// - Model trait（数据库操作）
```

**定义自定义用户模型:**

如果需要自定义字段，定义你自己的模型:

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

	// 添加自定义字段
	#[field(max_length = 50, null = true)]
	pub phone_number: Option<String>,
}
```

**Model属性宏:**

`#[model(...)]`属性自动生成:
- `Model` trait的实现（包含`#[derive(Model)]`功能）
- 类型安全的字段访问器: `User::field_email()`、`User::field_username()`等
- 全局模型注册表注册
- 复合主键支持

**注意:** 使用`#[model(...)]`时，不需要单独添加`#[derive(Model)]`，
它由`#[model(...)]`属性自动应用。

**字段属性:**
- `#[field(primary_key = true)]` - 标记为主键
- `#[field(max_length = 255)]` - 设置字符串字段的最大长度
- `#[field(default = value)]` - 设置默认值
- `#[field(auto_now_add = true)]` - 创建时自动填充时间戳
- `#[field(auto_now = true)]` - 保存时自动更新时间戳
- `#[field(null = true)]` - 允许NULL值
- `#[field(unique = true)]` - 强制唯一性约束

完整的字段属性列表请参阅[字段属性指南](docs/field_attributes.md)。

生成的字段访问器在查询中启用类型安全的字段引用:

```rust
// 由#[model(...)]为DefaultUser生成
impl DefaultUser {
	pub const fn field_id() -> FieldRef<DefaultUser, Uuid> { FieldRef::new("id") }
	pub const fn field_username() -> FieldRef<DefaultUser, String> { FieldRef::new("username") }
	pub const fn field_email() -> FieldRef<DefaultUser, String> { FieldRef::new("email") }
	pub const fn field_is_active() -> FieldRef<DefaultUser, bool> { FieldRef::new("is_active") }
	pub const fn field_is_staff() -> FieldRef<DefaultUser, bool> { FieldRef::new("is_staff") }
	pub const fn field_date_joined() -> FieldRef<DefaultUser, DateTime<Utc>> { FieldRef::new("date_joined") }
	// ... 其他字段
}
```

**高级查询示例:**

```rust
use reinhardt::prelude::*;
use reinhardt::DefaultUser;

// 使用类型安全字段引用的Django风格F/Q对象查询
async fn complex_user_query() -> Result<Vec<DefaultUser>, Box<dyn std::error::Error>> {
	// 使用类型安全字段引用的Q对象（使用生成的字段访问器）
	let active_query = Q::new()
		.field("is_active").eq(true)
		.and(Q::new().field("date_joined").gte(Now::new()));

	// 使用类型安全字段引用的数据库函数
	let email_lower = Lower::new(DefaultUser::field_email().into());
	let username_upper = Upper::new(DefaultUser::field_username().into());

	// 使用字段访问器的聚合
	let user_count = Aggregate::count(DefaultUser::field_id().into());
	let latest_joined = Aggregate::max(DefaultUser::field_date_joined().into());

	// 用于排名的窗口函数
	let rank_by_join_date = Window::new()
		.partition_by(vec![DefaultUser::field_is_active().into()])
		.order_by(vec![(DefaultUser::field_date_joined().into(), "DESC")])
		.function(RowNumber::new());

	todo!("Execute query with these components")
}

// 事务支持
async fn create_user_with_transaction(
	conn: &DatabaseConnection,
	user_data: CreateUserRequest
) -> Result<User, Box<dyn std::error::Error>> {
	// 出错时自动回滚的事务
	transaction(conn, |_tx| async move {
		let user = User::create(user_data).await?;
		log_user_creation(&user).await?;
		Ok(user)
	}).await
}
```

**注意**: Reinhardt使用reinhardt-query进行SQL操作。`#[derive(Model)]`宏自动生成Model trait实现、类型安全的字段访问器和全局模型注册表注册。

在`src/config/apps.rs`中注册:

```rust
// src/config/apps.rs
use reinhardt::installed_apps;

// installed_apps!宏生成:
// - 每个app变体的InstalledApp枚举
// - 转换trait的实现（From、Into、Display）
// - app配置和发现的注册表
//
// 注意: 与Django的INSTALLED_APPS不同，此宏仅用于用户apps。
// 内置框架功能（auth、sessions、admin等）通过
// Cargo功能标志启用，而不是通过installed_apps!。
//
// 示例:
// [dependencies]
// reinhardt = { version = "0.1", features = ["auth", "sessions", "admin"] }
//
// 这启用:
// - 迁移、管理面板等的自动app发现
// - 代码中的类型安全app引用
// - 集中的app配置
installed_apps! {
	users: "users",
}

pub fn get_installed_apps() -> Vec<String> {
	InstalledApp::all_apps()
}
```

### 使用认证

Reinhardt提供Django风格的用户模型，带有`BaseUser`和`FullUser` traits，以及通过`UserManager`的全面用户管理。

**注意:** Reinhardt包含内置的`DefaultUser`实现。你可以直接使用它或如下所示定义自己的用户模型。

**用户管理示例:**

```rust
use reinhardt::prelude::*;

// 使用UserManager创建和管理用户
async fn manage_users() -> Result<(), Box<dyn std::error::Error>> {
	let hasher = Argon2Hasher::new();
	let user_manager = UserManager::new(hasher);

	// 创建新用户
	let user = user_manager.create_user(CreateUserData {
		username: "alice".to_string(),
		email: "alice@example.com".to_string(),
		password: "secure_password".to_string(),
		first_name: Some("Alice".to_string()),
		last_name: Some("Smith".to_string()),
	}).await?;

	// 更新用户信息
	user_manager.update_user(user.id, UpdateUserData {
		email: Some("alice.smith@example.com".to_string()),
		is_active: Some(true),
		..Default::default()
	}).await?;

	// 管理组和权限
	let group_manager = GroupManager::new();
	let editors = group_manager.create_group(CreateGroupData {
		name: "editors".to_string(),
	}).await?;

	// 分配对象级权限
	let permission = ObjectPermission::new("edit", user.id, article.id);
	let perm_checker = ObjectPermissionChecker::new();
	if perm_checker.has_permission(&user, "edit", &article).await? {
		// 用户可以编辑文章
	}

	Ok(())
}
```

在`users/models.rs`中使用内置的`DefaultUser`:

```rust
// users/models.rs
use reinhardt::DefaultUser;

// 将DefaultUser重新导出为你的User类型
pub type User = DefaultUser;

// DefaultUser已实现:
// - BaseUser trait（认证方法）
// - FullUser trait（username、email、first_name、last_name等）
// - PermissionsMixin trait（权限管理）
// - Model trait（数据库操作）
```

**对于自定义用户模型:**

如果需要超出DefaultUser的额外字段，定义你自己的:

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

	// 自定义字段
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

在app的`views/profile.rs`中使用JWT认证:

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
	// 从Authorization头提取JWT令牌
	let auth_header = req.headers.get("authorization")
		.and_then(|h| h.to_str().ok())
		.ok_or("Missing Authorization header")?;

	let token = auth_header.strip_prefix("Bearer ")
		.ok_or("Invalid Authorization header format")?;

	// 验证令牌并获取用户ID
	let jwt_auth = JwtAuth::new(b"your-secret-key");
	let claims = jwt_auth.verify_token(token)?;

	// 使用claims.user_id从数据库加载用户
	let user = User::find_by_id(&db, &claims.user_id).await?;

	// 检查用户是否活跃
	if !user.is_active() {
		return Err("User account is inactive".into());
	}

	// 返回用户配置文件为JSON
	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

### 端点定义

Reinhardt使用HTTP方法装饰器定义端点:

#### HTTP方法装饰器

使用`#[get]`、`#[post]`、`#[put]`、`#[delete]`定义路由:

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
- 编译时路径验证
- 简洁语法
- 自动HTTP方法绑定
- 通过`#[inject]`支持依赖注入

#### 使用依赖注入

将HTTP方法装饰器与`#[inject]`结合进行自动依赖注入:

```rust
use reinhardt::{get, Request, Response, StatusCode, ViewResult};
use reinhardt::db::DatabaseConnection;
use std::sync::Arc;

#[get("/users/{id}/", name = "get_user")]
pub async fn get_user(
	req: Request,
	#[inject] db: Arc<DatabaseConnection>,  // 自动注入
) -> ViewResult<Response> {
	let id = req.path_params.get("id")
		.ok_or("Missing id")?
		.parse::<i64>()?;

	// 使用注入的数据库连接
	let user = db.query("SELECT * FROM users WHERE id = $1")
		.bind(id)
		.fetch_one()
		.await?;

	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

**依赖注入功能:**
- 通过`#[inject]`属性自动依赖注入
- 通过`#[inject(cache = false)]`控制缓存
- FastAPI启发的依赖注入系统
- 与HTTP方法装饰器无缝协作

**返回类型:**

所有视图函数使用`ViewResult<T>`作为返回类型:

```rust
use reinhardt::ViewResult;  // 预定义结果类型
```

### 使用参数提取

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
	// 从请求提取路径参数
	let id = req.path_params.get("id")
		.ok_or("Missing id parameter")?
		.parse::<i64>()
		.map_err(|_| "Invalid id format")?;

	// 提取查询参数（例如 ?include_inactive=true）
	let include_inactive = req.query_params.get("include_inactive")
		.and_then(|v| v.parse::<bool>().ok())
		.unwrap_or(false);

	// 使用注入的连接从数据库获取用户
	let user = User::find_by_id(&db, id).await?;

	// 如需检查活跃状态
	if !include_inactive && !user.is_active {
		return Err("User is inactive".into());
	}

	// 返回JSON
	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

在`urls.rs`中注册带路径参数的路由:

```rust
// users/urls.rs
use reinhardt::ServerRouter;

use super::views;

pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::get_user)  // 路径在#[get("/users/{id}/")]中定义
}
```

### 使用序列化器和验证

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
	// 解析请求体
	let body_bytes = std::mem::take(&mut req.body);
	let create_req: CreateUserRequest = serde_json::from_slice(&body_bytes)?;

	// 验证请求
	create_req.validate()?;

	// 创建用户
	let mut user = User {
		id: 0, // 将由数据库设置
		username: create_req.username,
		email: create_req.email,
		password_hash: None,
		is_active: true,
		created_at: Utc::now(),
	};

	// 使用BaseUser trait哈希密码
	user.set_password(&create_req.password)?;

	// 使用注入的连接保存到数据库
	user.save(&db).await?;

	// 转换为响应
	let response_data = UserResponse::from(user);
	let json = serde_json::to_string(&response_data)?;

	Ok(Response::new(StatusCode::CREATED)
		.with_body(json))
}
```

## 可用组件

Reinhardt提供可混合搭配的模块化组件:

| 组件               | Crate名称                  | 功能                                        |
|---------------------|---------------------------|---------------------------------------------|
| **核心**            |                           |                                             |
| 核心类型            | `reinhardt-core`          | 核心traits、类型、宏（Model、endpoint）     |
| HTTP和路由          | `reinhardt-http`          | Request/Response、HTTP处理                  |
| URL路由             | `reinhardt-urls`          | 函数式和类式路由                            |
| 服务器              | `reinhardt-server`        | HTTP服务器实现                              |
| 中间件              | `reinhardt-dispatch`      | 中间件链、信号分发                          |
| 配置                | `reinhardt-conf`          | 设置管理、环境加载                          |
| 命令                | `reinhardt-commands`      | 管理CLI工具（startproject等）               |
| 快捷方式            | `reinhardt-shortcuts`     | 常用工具函数                                |
| **数据库**          |                           |                                             |
| ORM                 | `reinhardt-db`            | reinhardt-query集成                         |
| **认证**            |                           |                                             |
| Auth                | `reinhardt-auth`          | JWT、Token、Session、Basic认证、用户模型   |
| **REST API**        |                           |                                             |
| 序列化器            | `reinhardt-rest`          | serde/validator集成、ViewSets               |
| **表单**            |                           |                                             |
| 表单                | `reinhardt-forms`         | 表单处理和验证                              |
| **高级功能**        |                           |                                             |
| 管理面板            | `reinhardt-admin`         | Django风格管理界面                          |
| 插件系统            | `reinhardt-dentdelion`    | 静态和WASM插件支持、CLI管理                 |
| 后台任务            | `reinhardt-tasks`         | 任务队列（Redis、RabbitMQ、SQLite）         |
| GraphQL             | `reinhardt-graphql`       | Schema生成、订阅                            |
| WebSockets          | `reinhardt-websockets`    | 实时通信                                    |
| i18n                | `reinhardt-i18n`          | 多语言支持                                  |
| **测试**            |                           |                                             |
| 测试工具            | `reinhardt-test`          | 测试助手、fixtures、TestContainers          |

**各crate内的详细功能标志，请参阅[功能标志指南](https://reinhardt-web.dev/docs/feature-flags/)。**

---

## 文档

- 📚 [入门指南](https://reinhardt-web.dev/quickstart/getting-started/) - 初学者分步教程
- 🎛️ [功能标志指南](https://reinhardt-web.dev/docs/feature-flags/) - 通过细粒度功能控制优化构建
- 📖 [API参考](https://docs.rs/reinhardt)（即将推出）
- 📝 [教程](https://reinhardt-web.dev/quickstart/tutorials/) - 通过构建真实应用学习

**AI助手请参阅**: 项目特定的编码标准、测试指南和开发约定请参阅[CLAUDE.md](CLAUDE.md)。

## 💬 获取帮助

Reinhardt是一个社区驱动的项目。以下是获取帮助的途径:

- 💬 **Discord**: 加入我们的Discord服务器进行实时聊天（即将推出）
- 💭 **GitHub Discussions**: [提问和分享想法](https://github.com/kent8192/reinhardt-web/discussions)
- 🐛 **Issues**: [报告bug](https://github.com/kent8192/reinhardt-web/issues)
- 📖 **文档**: [阅读指南](https://reinhardt-web.dev/)

提问前，请查看:

- ✅ [入门指南](https://reinhardt-web.dev/quickstart/getting-started/)
- ✅ [Examples](https://github.com/kent8192/reinhardt-web/tree/main/examples/)
- ✅ 现有的GitHub Issues和Discussions

## 🤝 贡献

我们欢迎贡献！请阅读[贡献指南](../../CONTRIBUTING.md)开始。

**快速链接**:

- [开发设置](../../CONTRIBUTING.md#development-setup)
- [测试指南](../../CONTRIBUTING.md#testing-guidelines)
- [提交指南](../../CONTRIBUTING.md#commit-guidelines)

## ⭐ Star 趋势

<a href="https://star-history.com/#kent8192/reinhardt-web&Date">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date&theme=dark" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date" />
   <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date" width="600" />
 </picture>
</a>

## 许可证

本项目基于 [BSD 3-Clause License](../../LICENSE) 许可。

### 第三方归属

本项目受以下项目启发:

- [Django](https://www.djangoproject.com/)（BSD 3-Clause许可证）
- [Django REST Framework](https://www.django-rest-framework.org/)（BSD 3-Clause许可证）
- [FastAPI](https://fastapi.tiangolo.com/)（MIT许可证）
- [SQLAlchemy](https://www.sqlalchemy.org/)（MIT许可证）

完整归属请参阅[THIRD-PARTY-NOTICES](../../THIRD-PARTY-NOTICES)。

**注意:** 本项目不隶属于Django Software Foundation、Encode OSS Ltd.、Sebastián Ramírez（FastAPI作者）或Michael Bayer（SQLAlchemy作者），也未获得其认可。
