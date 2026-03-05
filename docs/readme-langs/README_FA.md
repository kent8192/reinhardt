<div align="center">
  <img src="../../branding/logo.png" alt="Reinhardt Logo" width="200"/>

  <h1>Reinhardt</h1>

  <h3>🦀 فریم‌ورک چندلیتیک با باتری‌های کامل</h3>

  <p><strong>یک فریم‌ورک API تمام‌پشته قابل ترکیب برای Rust</strong></p>
  <p>با <em>تمام</em> قدرت فلسفه "باتری‌های کامل" Django بسازید،<br/>
  یا <em>فقط</em> آنچه نیاز دارید را ترکیب کنید—انتخاب شما، راه شما.</p>

🌐 [English](../../README.md) | [日本語](README_JA.md) | [简体中文](README_ZH_CN.md) | [繁體中文](README_ZH_TW.md) | [Русский](README_RU.md) | [Українська](README_UK.md) | **فارسی** | [العربية](README_AR.md)

[![Crates.io](https://img.shields.io/crates/v/reinhardt-web.svg)](https://crates.io/crates/reinhardt-web)
[![Documentation](https://docs.rs/reinhardt-web/badge.svg)](https://docs.rs/reinhardt-web)
[![License](https://img.shields.io/badge/license-BSD--3--Clause-blue.svg)](../../LICENSE)
[![codecov](https://codecov.io/gh/kent8192/reinhardt-web/graph/badge.svg)](https://codecov.io/gh/kent8192/reinhardt-web)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/kent8192/reinhardt-web)

</div>

---

## 📍 ناوبری سریع

شاید به دنبال این موارد باشید:

- 🌐 [وب‌سایت رسمی](https://reinhardt-web.dev) - مستندات، آموزش‌ها و راهنماها
- 🚀 [شروع سریع](#شروع-سریع) - راه‌اندازی در ۵ دقیقه
- 📦 [گزینه‌های نصب](#نصب) - نوع خود را انتخاب کنید: Micro، Standard یا Full
- 📚 [راهنمای شروع کار](https://reinhardt-web.dev/quickstart/getting-started/) - آموزش گام به گام
- 🎛️ [پرچم‌های ویژگی](https://reinhardt-web.dev/docs/feature-flags/) - تنظیم دقیق ساخت
- 📖 [مستندات API](https://docs.rs/reinhardt-web) - مرجع کامل API
- 💬 [انجمن و پشتیبانی](#دریافت-کمک) - از انجمن کمک بگیرید

## چرا Reinhardt؟

**Polylithic = Poly (بسیار) + Lithic (بلوک‌های ساختمانی)**
برخلاف فریم‌ورک‌های یکپارچه که شما را مجبور به استفاده از همه چیز می‌کنند، Reinhardt به شما اجازه می‌دهد پشته کامل خود را از اجزای مستقل و به خوبی تست شده بسازید.

Reinhardt بهترین‌ها را از سه دنیا گرد هم می‌آورد:

| الهام‌بخش          | چه چیزی قرض گرفتیم                                     | چه چیزی بهبود دادیم                                  |
|--------------------|--------------------------------------------------------|------------------------------------------------------|
| 🐍 **Django**      | فلسفه باتری‌های کامل، طراحی ORM، پنل مدیریت            | پرچم‌های ویژگی برای ساخت‌های قابل ترکیب، ایمنی نوع Rust |
| 🎯 **Django REST** | سریال‌سازها، ViewSets، مجوزها                           | اعتبارسنجی زمان کامپایل، انتزاع‌های بدون هزینه        |
| ⚡ **FastAPI**      | سیستم DI، OpenAPI خودکار                               | عملکرد بومی Rust، بدون سربار زمان اجرا               |
| 🗄️ **SQLAlchemy** | الگوهای QuerySet، مدیریت روابط                          | سازنده کوئری ایمن از نظر نوع، اعتبارسنجی زمان کامپایل |

**نتیجه**: فریم‌ورکی آشنا برای توسعه‌دهندگان Python، اما با عملکرد و تضمین‌های امنیتی Rust.

## ✨ ویژگی‌های کلیدی

- **ORM ایمن از نظر نوع** با اعتبارسنجی زمان کامپایل (reinhardt-query)
- **سریال‌سازهای قدرتمند** با اعتبارسنجی خودکار (serde + validator)
- **DI به سبک FastAPI** با تزریق وابستگی ایمن از نظر نوع و کش
- **ViewSets** برای توسعه سریع CRUD API
- **احراز هویت چندگانه** (JWT، Token، Session، Basic) با صفات BaseUser/FullUser
- **پنل مدیریت** با رابط مدیریت مدل خودکار
- **دستورات مدیریت** برای مهاجرت، فایل‌های استاتیک و موارد دیگر
- **پشتیبانی GraphQL و WebSocket** برای برنامه‌های بلادرنگ
- **صفحه‌بندی، فیلتر، محدودیت نرخ** داخلی
- **سیگنال‌ها** برای معماری رویداد‌محور

لیست کامل را در [اجزای موجود](#اجزای-موجود) و نمونه‌ها را در [راهنمای شروع کار](https://reinhardt-web.dev/quickstart/getting-started/) ببینید.

## نصب

Reinhardt یک فریم‌ورک ماژولار است. نقطه شروع خود را انتخاب کنید:

**نکته درباره نام‌گذاری کریت:**
کریت اصلی Reinhardt در crates.io با نام `reinhardt-web` منتشر شده است، اما شما آن را با استفاده از ویژگی `package` به عنوان `reinhardt` در کد خود وارد می‌کنید.

### پیش‌فرض: کامل‌ویژگی (باتری‌های کامل) ⚠️ پیش‌فرض جدید

همه ویژگی‌ها بدون تنظیمات:

```toml
[dependencies]
# به عنوان 'reinhardt' وارد می‌شود، با نام 'reinhardt-web' منتشر شده
# پیش‌فرض همه ویژگی‌ها را فعال می‌کند (بسته کامل)
reinhardt = { version = "0.1.0-rc.1", package = "reinhardt-web" }
```

**شامل:** Database، Auth، REST API، Admin، GraphQL، WebSockets، Cache، i18n، Mail، Sessions، Static Files، Storage

**باینری**: ~50+ مگابایت | **کامپایل**: کندتر، اما همه چیز از جعبه کار می‌کند

سپس در کد استفاده کنید:
```rust
use reinhardt::prelude::*;
use reinhardt::{Request, Response, StatusCode};
```

### گزینه ۱: راه‌اندازی استاندارد (متعادل)

برای اکثر پروژه‌هایی که به همه ویژگی‌ها نیاز ندارند:

```toml
[dependencies]
reinhardt = { version = "0.1.0-rc.1", package = "reinhardt-web", default-features = false, features = ["standard"] }
```

**شامل:** Core، Database (PostgreSQL)، REST API، Auth، Middleware، Pages (فرانت‌اند WASM با SSR)

**باینری**: ~20-30 مگابایت | **کامپایل**: متوسط

### گزینه ۲: میکروسرویس‌ها (راه‌اندازی حداقلی)

سبک و سریع، مناسب برای APIهای ساده:

```toml
[dependencies]
reinhardt = { version = "0.1.0-rc.1", package = "reinhardt-web", default-features = false, features = ["minimal"] }
```

**شامل:** HTTP، مسیریابی، DI، استخراج پارامتر، سرور

**باینری**: ~5-10 مگابایت | **کامپایل**: بسیار سریع

### گزینه ۳: پشته سفارشی خود را بسازید

فقط اجزای مورد نیاز را نصب کنید:

```toml
[dependencies]
# اجزای اصلی
reinhardt-http = "0.1.0-rc.1"
reinhardt-urls = "0.1.0-rc.1"

# اختیاری: پایگاه داده
reinhardt-db = "0.1.0-rc.1"

# اختیاری: احراز هویت
reinhardt-auth = "0.1.0-rc.1"

# اختیاری: ویژگی‌های REST API
reinhardt-rest = "0.1.0-rc.1"

# اختیاری: پنل مدیریت
reinhardt-admin = "0.1.0-rc.1"

# اختیاری: ویژگی‌های پیشرفته
reinhardt-graphql = "0.1.0-rc.1"
reinhardt-websockets = "0.1.0-rc.1"
```

**📖 برای لیست کامل کریت‌ها و پرچم‌های ویژگی موجود، [راهنمای پرچم‌های ویژگی](https://reinhardt-web.dev/docs/feature-flags/) را ببینید.**

## شروع سریع

### ۱. ابزار Reinhardt Admin را نصب کنید

```bash
cargo install reinhardt-admin-cli
```

### ۲. پروژه جدید ایجاد کنید

```bash
# ایجاد پروژه RESTful API (پیش‌فرض)
reinhardt-admin startproject my-api
cd my-api
```

این ساختار کامل پروژه را تولید می‌کند:

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

**جایگزین: ایجاد پروژه reinhardt-pages (WASM + SSR)**

برای فرانت‌اند مدرن WASM با SSR:

```bash
# ایجاد پروژه pages
reinhardt-admin startproject my-app --with-pages
cd my-app

# نصب ابزارهای ساخت WASM (فقط بار اول)
cargo make install-wasm-tools

# ساخت WASM و راه‌اندازی سرور توسعه
cargo make dev
# به http://127.0.0.1:8000/ مراجعه کنید
```

### ۳. سرور توسعه را اجرا کنید

```bash
# با استفاده از دستور manage
cargo run --bin manage runserver

# سرور در http://127.0.0.1:8000 شروع به کار می‌کند
```

**پشتیبانی از بارگذاری مجدد خودکار:**

برای بارگذاری مجدد خودکار هنگام تغییر کد (نیاز به bacon):

```bash
# نصب bacon
cargo install --locked bacon

# اجرا با بارگذاری مجدد خودکار
bacon runserver

# یا از cargo make استفاده کنید
cargo make watch

# برای تست‌ها
bacon test
```

### ۴. اولین برنامه خود را ایجاد کنید

```bash
# ایجاد برنامه RESTful API (پیش‌فرض)
cargo run --bin manage startapp users

# یا نوع را به صراحت مشخص کنید
cargo run --bin manage startapp users --restful

# ایجاد برنامه Pages (WASM + SSR)
cargo run --bin manage startapp dashboard --with-pages
```

این ساختار برنامه را ایجاد می‌کند:

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

### ۵. مسیرها را ثبت کنید

`urls.rs` برنامه خود را ویرایش کنید:

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

در `src/config/urls.rs` شامل کنید:

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

ماکرو ویژگی `#[routes]` به طور خودکار این تابع را با فریم‌ورک برای کشف از طریق کریت `inventory` ثبت می‌کند.

**نکته:** `reinhardt::prelude` شامل انواع متداول است. صادرات‌های اصلی:

**همیشه موجود:**
- مسیریابی و نماهای اصلی: `Router`، `DefaultRouter`، `ServerRouter`، `View`، `ListView`، `DetailView`
- ViewSets: `ViewSet`، `ModelViewSet`، `ReadOnlyModelViewSet`
- HTTP: `StatusCode`

**وابسته به ویژگی:**
- **ویژگی `core`**: `Request`، `Response`، `Handler`، `Middleware`، سیگنال‌ها (`post_save`، `pre_save` و غیره)
- **ویژگی `database`**: `Model`، `DatabaseConnection`، `F`، `Q`، `Transaction`، `atomic`، توابع پایگاه داده (`Concat`، `Upper`، `Lower`، `Now`، `CurrentDate`)، توابع پنجره‌ای (`Window`، `RowNumber`، `Rank`، `DenseRank`)، محدودیت‌ها (`UniqueConstraint`، `CheckConstraint`، `ForeignKeyConstraint`)
- **ویژگی `auth`**: `User`، `UserManager`، `GroupManager`، `Permission`، `ObjectPermission`
- **ویژگی‌های `minimal`، `standard` یا `di`**: `Body`، `Cookie`، `Header`، `Json`، `Path`، `Query`
- **ویژگی `rest`**: سریال‌سازها، پارسرها، صفحه‌بندی، محدودیت نرخ، نسخه‌بندی
- **ویژگی `admin`**: اجزای پنل مدیریت
- **ویژگی `cache`**: `Cache`، `InMemoryCache`
- **ویژگی `sessions`**: `Session`، `AuthenticationMiddleware`

لیست کامل را در [راهنمای پرچم‌های ویژگی](https://reinhardt-web.dev/docs/feature-flags/) ببینید.

راهنمای کامل گام به گام را در [راهنمای شروع کار](https://reinhardt-web.dev/quickstart/getting-started/) ببینید.

## 🎓 با مثال یاد بگیرید

### با پایگاه داده

پایگاه داده را در `settings/base.toml` تنظیم کنید:

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

تنظیمات به طور خودکار در `src/config/settings.rs` بارگذاری می‌شوند:

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

**منابع متغیر محیطی:**

Reinhardt دو نوع منبع متغیر محیطی با اولویت‌های مختلف ارائه می‌دهد:

- **`EnvSource`** (اولویت: 100) - متغیرهای محیطی با اولویت بالا که فایل‌های TOML را لغو می‌کنند
  ```rust
  .add_source(EnvSource::new().with_prefix("REINHARDT_"))
  ```

- **`LowPriorityEnvSource`** (اولویت: 40) - متغیرهای محیطی با اولویت پایین که به فایل‌های TOML بازمی‌گردند
  ```rust
  .add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
  ```

**ترتیب اولویت**:
- با `EnvSource`: متغیرهای محیطی > `{profile}.toml` > `base.toml` > پیش‌فرض‌ها
- با `LowPriorityEnvSource` (نشان داده شده در بالا): `{profile}.toml` > `base.toml` > متغیرهای محیطی > پیش‌فرض‌ها

وقتی متغیرهای محیطی باید همیشه اولویت داشته باشند `EnvSource` را انتخاب کنید (مثلاً استقرار production).
وقتی فایل‌های TOML باید منبع اصلی پیکربندی باشند `LowPriorityEnvSource` را انتخاب کنید (مثلاً توسعه).

برای جزئیات بیشتر [مستندات تنظیمات](docs/SETTINGS_DOCUMENT.md) را ببینید.

**استفاده از DefaultUser داخلی:**

Reinhardt یک پیاده‌سازی `DefaultUser` آماده استفاده ارائه می‌دهد (نیاز به ویژگی `argon2-hasher`):

```rust
// users/models.rs
use reinhardt::prelude::*;
use reinhardt::DefaultUser;

// DefaultUser را به عنوان User برنامه خود صادر مجدد کنید
pub type User = DefaultUser;

// DefaultUser شامل:
// - id: Uuid (کلید اصلی)
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

// DefaultUser پیاده‌سازی می‌کند:
// - صفت BaseUser (متدهای احراز هویت)
// - صفت FullUser (اطلاعات کامل کاربر)
// - صفت PermissionsMixin (مدیریت مجوزها)
// - صفت Model (عملیات پایگاه داده)
```

**تعریف مدل‌های کاربر سفارشی:**

اگر به فیلدهای سفارشی نیاز دارید، مدل خود را تعریف کنید:

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

	// اضافه کردن فیلدهای سفارشی
	#[field(max_length = 50, null = true)]
	pub phone_number: Option<String>,
}
```

**ماکرو ویژگی Model:**

ویژگی `#[model(...)]` به طور خودکار تولید می‌کند:
- پیاده‌سازی صفت `Model` (شامل قابلیت `#[derive(Model)]`)
- دسترسی‌دهنده‌های فیلد ایمن از نظر نوع: `User::field_email()`، `User::field_username()` و غیره
- ثبت در رجیستری مدل جهانی
- پشتیبانی از کلیدهای اصلی ترکیبی

**نکته:** هنگام استفاده از `#[model(...)]`، نیازی به اضافه کردن جداگانه `#[derive(Model)]` ندارید،
زیرا به طور خودکار توسط ویژگی `#[model(...)]` اعمال می‌شود.

**ویژگی‌های فیلد:**
- `#[field(primary_key = true)]` - علامت‌گذاری به عنوان کلید اصلی
- `#[field(max_length = 255)]` - تنظیم حداکثر طول برای فیلدهای رشته‌ای
- `#[field(default = value)]` - تنظیم مقدار پیش‌فرض
- `#[field(auto_now_add = true)]` - پر کردن خودکار timestamp هنگام ایجاد
- `#[field(auto_now = true)]` - به‌روزرسانی خودکار timestamp هنگام ذخیره
- `#[field(null = true)]` - اجازه مقادیر NULL
- `#[field(unique = true)]` - اعمال محدودیت یکتایی

لیست کامل ویژگی‌های فیلد را در [راهنمای ویژگی‌های فیلد](docs/field_attributes.md) ببینید.

دسترسی‌دهنده‌های فیلد تولید شده امکان ارجاع ایمن از نظر نوع به فیلدها در کوئری‌ها را فراهم می‌کنند:

```rust
// تولید شده توسط #[model(...)] برای DefaultUser
impl DefaultUser {
	pub const fn field_id() -> FieldRef<DefaultUser, Uuid> { FieldRef::new("id") }
	pub const fn field_username() -> FieldRef<DefaultUser, String> { FieldRef::new("username") }
	pub const fn field_email() -> FieldRef<DefaultUser, String> { FieldRef::new("email") }
	pub const fn field_is_active() -> FieldRef<DefaultUser, bool> { FieldRef::new("is_active") }
	pub const fn field_is_staff() -> FieldRef<DefaultUser, bool> { FieldRef::new("is_staff") }
	pub const fn field_date_joined() -> FieldRef<DefaultUser, DateTime<Utc>> { FieldRef::new("date_joined") }
	// ... سایر فیلدها
}
```

**نمونه‌های کوئری پیشرفته:**

```rust
use reinhardt::prelude::*;
use reinhardt::DefaultUser;

// کوئری‌های اشیاء F/Q به سبک Django با ارجاعات فیلد ایمن از نظر نوع
async fn complex_user_query() -> Result<Vec<DefaultUser>, Box<dyn std::error::Error>> {
	// اشیاء Q با ارجاعات فیلد ایمن از نظر نوع (با استفاده از دسترسی‌دهنده‌های فیلد تولید شده)
	let active_query = Q::new()
		.field("is_active").eq(true)
		.and(Q::new().field("date_joined").gte(Now::new()));

	// توابع پایگاه داده با ارجاعات فیلد ایمن از نظر نوع
	let email_lower = Lower::new(DefaultUser::field_email().into());
	let username_upper = Upper::new(DefaultUser::field_username().into());

	// تجمیع‌ها با استفاده از دسترسی‌دهنده‌های فیلد
	let user_count = Aggregate::count(DefaultUser::field_id().into());
	let latest_joined = Aggregate::max(DefaultUser::field_date_joined().into());

	// توابع پنجره‌ای برای رتبه‌بندی
	let rank_by_join_date = Window::new()
		.partition_by(vec![DefaultUser::field_is_active().into()])
		.order_by(vec![(DefaultUser::field_date_joined().into(), "DESC")])
		.function(RowNumber::new());

	todo!("Execute query with these components")
}

// پشتیبانی از تراکنش
async fn create_user_with_transaction(
	conn: &DatabaseConnection,
	user_data: CreateUserRequest
) -> Result<User, Box<dyn std::error::Error>> {
	// تراکنش با بازگشت خودکار در صورت خطا
	transaction(conn, |_tx| async move {
		let user = User::create(user_data).await?;
		log_user_creation(&user).await?;
		Ok(user)
	}).await
}
```

**نکته**: Reinhardt از reinhardt-query برای عملیات SQL استفاده می‌کند. ماکرو `#[derive(Model)]` به طور خودکار پیاده‌سازی‌های صفت Model، دسترسی‌دهنده‌های فیلد ایمن از نظر نوع و ثبت در رجیستری مدل جهانی را تولید می‌کند.

در `src/config/apps.rs` ثبت کنید:

```rust
// src/config/apps.rs
use reinhardt::installed_apps;

// ماکرو installed_apps! تولید می‌کند:
// - یک enum InstalledApp با واریانت‌هایی برای هر برنامه
// - پیاده‌سازی صفات تبدیل (From، Into، Display)
// - یک رجیستری برای پیکربندی و کشف برنامه
//
// نکته: برخلاف INSTALLED_APPS در Django، این ماکرو فقط برای برنامه‌های کاربر است.
// ویژگی‌های داخلی فریم‌ورک (auth، sessions، admin و غیره) از طریق
// پرچم‌های ویژگی Cargo فعال می‌شوند، نه از طریق installed_apps!.
//
// مثال:
// [dependencies]
// reinhardt = { version = "0.1", features = ["auth", "sessions", "admin"] }
//
// این فعال می‌کند:
// - کشف خودکار برنامه برای مهاجرت‌ها، پنل مدیریت و غیره
// - ارجاعات برنامه ایمن از نظر نوع در سراسر کد شما
// - پیکربندی متمرکز برنامه
installed_apps! {
	users: "users",
}

pub fn get_installed_apps() -> Vec<String> {
	InstalledApp::all_apps()
}
```

### با احراز هویت

Reinhardt مدل‌های کاربر به سبک Django با صفات `BaseUser` و `FullUser`، همراه با مدیریت جامع کاربر از طریق `UserManager` ارائه می‌دهد.

**نکته:** Reinhardt شامل یک پیاده‌سازی `DefaultUser` داخلی است. می‌توانید مستقیماً از آن استفاده کنید یا مدل کاربر خود را مطابق شکل زیر تعریف کنید.

**نمونه مدیریت کاربر:**

```rust
use reinhardt::prelude::*;

// ایجاد و مدیریت کاربران با UserManager
async fn manage_users() -> Result<(), Box<dyn std::error::Error>> {
	let hasher = Argon2Hasher::new();
	let user_manager = UserManager::new(hasher);

	// ایجاد یک کاربر جدید
	let user = user_manager.create_user(CreateUserData {
		username: "alice".to_string(),
		email: "alice@example.com".to_string(),
		password: "secure_password".to_string(),
		first_name: Some("Alice".to_string()),
		last_name: Some("Smith".to_string()),
	}).await?;

	// به‌روزرسانی اطلاعات کاربر
	user_manager.update_user(user.id, UpdateUserData {
		email: Some("alice.smith@example.com".to_string()),
		is_active: Some(true),
		..Default::default()
	}).await?;

	// مدیریت گروه‌ها و مجوزها
	let group_manager = GroupManager::new();
	let editors = group_manager.create_group(CreateGroupData {
		name: "editors".to_string(),
	}).await?;

	// تخصیص مجوزهای سطح شیء
	let permission = ObjectPermission::new("edit", user.id, article.id);
	let perm_checker = ObjectPermissionChecker::new();
	if perm_checker.has_permission(&user, "edit", &article).await? {
		// کاربر می‌تواند مقاله را ویرایش کند
	}

	Ok(())
}
```

از `DefaultUser` داخلی در `users/models.rs` استفاده کنید:

```rust
// users/models.rs
use reinhardt::DefaultUser;

// DefaultUser را به عنوان نوع User خود صادر مجدد کنید
pub type User = DefaultUser;

// DefaultUser قبلاً پیاده‌سازی کرده است:
// - صفت BaseUser (متدهای احراز هویت)
// - صفت FullUser (username، email، first_name، last_name و غیره)
// - صفت PermissionsMixin (مدیریت مجوزها)
// - صفت Model (عملیات پایگاه داده)
```

**برای مدل‌های کاربر سفارشی:**

اگر به فیلدهای اضافی فراتر از DefaultUser نیاز دارید، خودتان تعریف کنید:

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

	// فیلدهای سفارشی
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

از احراز هویت JWT در `views/profile.rs` برنامه خود استفاده کنید:

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
	// استخراج توکن JWT از هدر Authorization
	let auth_header = req.headers.get("authorization")
		.and_then(|h| h.to_str().ok())
		.ok_or("Missing Authorization header")?;

	let token = auth_header.strip_prefix("Bearer ")
		.ok_or("Invalid Authorization header format")?;

	// تأیید توکن و دریافت شناسه کاربر
	let jwt_auth = JwtAuth::new(b"your-secret-key");
	let claims = jwt_auth.verify_token(token)?;

	// بارگذاری کاربر از پایگاه داده با استفاده از claims.user_id
	let user = User::find_by_id(&db, &claims.user_id).await?;

	// بررسی فعال بودن کاربر
	if !user.is_active() {
		return Err("User account is inactive".into());
	}

	// بازگشت پروفایل کاربر به صورت JSON
	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

### تعریف نقطه پایانی

Reinhardt از دکوراتورهای متد HTTP برای تعریف نقاط پایانی استفاده می‌کند:

#### دکوراتورهای متد HTTP

از `#[get]`، `#[post]`، `#[put]`، `#[delete]` برای تعریف مسیرها استفاده کنید:

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

**ویژگی‌ها:**
- اعتبارسنجی مسیر در زمان کامپایل
- نحو مختصر
- اتصال خودکار متد HTTP
- پشتیبانی از تزریق وابستگی از طریق `#[inject]`

#### استفاده از تزریق وابستگی

دکوراتورهای متد HTTP را با `#[inject]` برای تزریق وابستگی خودکار ترکیب کنید:

```rust
use reinhardt::{get, Request, Response, StatusCode, ViewResult};
use reinhardt::db::DatabaseConnection;
use std::sync::Arc;

#[get("/users/{id}/", name = "get_user")]
pub async fn get_user(
	req: Request,
	#[inject] db: Arc<DatabaseConnection>,  // به طور خودکار تزریق می‌شود
) -> ViewResult<Response> {
	let id = req.path_params.get("id")
		.ok_or("Missing id")?
		.parse::<i64>()?;

	// استفاده از اتصال پایگاه داده تزریق شده
	let user = db.query("SELECT * FROM users WHERE id = $1")
		.bind(id)
		.fetch_one()
		.await?;

	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

**ویژگی‌های تزریق وابستگی:**
- تزریق وابستگی خودکار از طریق ویژگی `#[inject]`
- کنترل کش با `#[inject(cache = false)]`
- سیستم تزریق وابستگی الهام گرفته از FastAPI
- کار یکپارچه با دکوراتورهای متد HTTP

**نوع بازگشتی:**

همه توابع view از `ViewResult<T>` به عنوان نوع بازگشتی استفاده می‌کنند:

```rust
use reinhardt::ViewResult;  // نوع نتیجه از پیش تعریف شده
```

### با استخراج پارامتر

در `views/user.rs` برنامه خود:

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
	// استخراج پارامتر مسیر از درخواست
	let id = req.path_params.get("id")
		.ok_or("Missing id parameter")?
		.parse::<i64>()
		.map_err(|_| "Invalid id format")?;

	// استخراج پارامترهای query (مثلاً ?include_inactive=true)
	let include_inactive = req.query_params.get("include_inactive")
		.and_then(|v| v.parse::<bool>().ok())
		.unwrap_or(false);

	// دریافت کاربر از پایگاه داده با استفاده از اتصال تزریق شده
	let user = User::find_by_id(&db, id).await?;

	// بررسی وضعیت فعال در صورت نیاز
	if !include_inactive && !user.is_active {
		return Err("User is inactive".into());
	}

	// بازگشت به صورت JSON
	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

مسیر با پارامتر مسیر را در `urls.rs` ثبت کنید:

```rust
// users/urls.rs
use reinhardt::ServerRouter;

use super::views;

pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::get_user)  // مسیر در #[get("/users/{id}/")] تعریف شده
}
```

### با سریال‌سازها و اعتبارسنجی

در `serializers/user.rs` برنامه خود:

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

در `views/user.rs` برنامه خود:

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
	// تجزیه بدنه درخواست
	let body_bytes = std::mem::take(&mut req.body);
	let create_req: CreateUserRequest = serde_json::from_slice(&body_bytes)?;

	// اعتبارسنجی درخواست
	create_req.validate()?;

	// ایجاد کاربر
	let mut user = User {
		id: 0, // توسط پایگاه داده تنظیم می‌شود
		username: create_req.username,
		email: create_req.email,
		password_hash: None,
		is_active: true,
		created_at: Utc::now(),
	};

	// هش کردن رمز عبور با استفاده از صفت BaseUser
	user.set_password(&create_req.password)?;

	// ذخیره در پایگاه داده با استفاده از اتصال تزریق شده
	user.save(&db).await?;

	// تبدیل به پاسخ
	let response_data = UserResponse::from(user);
	let json = serde_json::to_string(&response_data)?;

	Ok(Response::new(StatusCode::CREATED)
		.with_body(json))
}
```

## اجزای موجود

Reinhardt اجزای ماژولار قابل ترکیب ارائه می‌دهد:

| جزء                | نام کریت                   | ویژگی‌ها                                    |
|---------------------|---------------------------|---------------------------------------------|
| **هسته**            |                           |                                             |
| انواع اصلی          | `reinhardt-core`          | صفات، انواع، ماکروهای اصلی (Model، endpoint)|
| HTTP و مسیریابی     | `reinhardt-http`          | Request/Response، مدیریت HTTP               |
| مسیریابی URL        | `reinhardt-urls`          | مسیرهای مبتنی بر تابع و کلاس               |
| سرور               | `reinhardt-server`        | پیاده‌سازی سرور HTTP                        |
| Middleware         | `reinhardt-dispatch`      | زنجیره Middleware، ارسال سیگنال            |
| پیکربندی           | `reinhardt-conf`          | مدیریت تنظیمات، بارگذاری محیط              |
| دستورات            | `reinhardt-commands`      | ابزارهای CLI مدیریت (startproject و غیره)  |
| میانبرها           | `reinhardt-shortcuts`     | توابع کمکی رایج                            |
| **پایگاه داده**     |                           |                                             |
| ORM                | `reinhardt-db`            | یکپارچگی reinhardt-query                   |
| **احراز هویت**      |                           |                                             |
| Auth               | `reinhardt-auth`          | JWT، Token، Session، Basic auth، مدل‌های User|
| **REST API**       |                           |                                             |
| سریال‌سازها         | `reinhardt-rest`          | یکپارچگی serde/validator، ViewSets          |
| **فرم‌ها**          |                           |                                             |
| فرم‌ها              | `reinhardt-forms`         | مدیریت و اعتبارسنجی فرم                    |
| **پیشرفته**         |                           |                                             |
| پنل مدیریت         | `reinhardt-admin`         | رابط مدیریت به سبک Django                   |
| سیستم پلاگین       | `reinhardt-dentdelion`    | پشتیبانی پلاگین استاتیک و WASM، مدیریت CLI |
| وظایف پس‌زمینه      | `reinhardt-tasks`         | صف وظایف (Redis، RabbitMQ، SQLite)         |
| GraphQL            | `reinhardt-graphql`       | تولید اسکیما، اشتراک‌ها                     |
| WebSockets         | `reinhardt-websockets`    | ارتباط بلادرنگ                              |
| i18n               | `reinhardt-i18n`          | پشتیبانی چندزبانه                           |
| **تست**            |                           |                                             |
| ابزارهای تست       | `reinhardt-test`          | کمک‌کننده‌های تست، فیکسچرها، TestContainers |

**برای پرچم‌های ویژگی دقیق در هر کریت، [راهنمای پرچم‌های ویژگی](https://reinhardt-web.dev/docs/feature-flags/) را ببینید.**

---

## مستندات

- 📚 [راهنمای شروع کار](https://reinhardt-web.dev/quickstart/getting-started/) - آموزش گام به گام برای مبتدیان
- 🎛️ [راهنمای پرچم‌های ویژگی](https://reinhardt-web.dev/docs/feature-flags/) - بهینه‌سازی ساخت با کنترل دقیق ویژگی
- 📖 [مرجع API](https://docs.rs/reinhardt) (به زودی)
- 📝 [آموزش‌ها](https://reinhardt-web.dev/quickstart/tutorials/) - یادگیری با ساخت برنامه‌های واقعی

**برای دستیاران AI**: [CLAUDE.md](CLAUDE.md) را برای استانداردهای کدنویسی خاص پروژه، راهنماهای تست و قراردادهای توسعه ببینید.

## 💬 دریافت کمک

Reinhardt یک پروژه مبتنی بر انجمن است. اینجا می‌توانید کمک بگیرید:

- 💬 **Discord**: به سرور Discord ما برای چت بلادرنگ بپیوندید (به زودی)
- 💭 **GitHub Discussions**: [سوال بپرسید و ایده‌ها را به اشتراک بگذارید](https://github.com/kent8192/reinhardt-web/discussions)
- 🐛 **Issues**: [گزارش باگ](https://github.com/kent8192/reinhardt-web/issues)
- 📖 **مستندات**: [راهنماها را بخوانید](../)

قبل از پرسیدن، لطفاً بررسی کنید:

- ✅ [راهنمای شروع کار](https://reinhardt-web.dev/quickstart/getting-started/)
- ✅ [مثال‌ها](../../examples/)
- ✅ Issues و Discussions موجود در GitHub

## 🤝 مشارکت

ما مشارکت‌ها را دوست داریم! لطفاً [راهنمای مشارکت](../../CONTRIBUTING.md) را برای شروع بخوانید.

**لینک‌های سریع**:

- [راه‌اندازی توسعه](../../CONTRIBUTING.md#development-setup)
- [راهنمای تست](../../CONTRIBUTING.md#testing-guidelines)
- [راهنمای کامیت](../../CONTRIBUTING.md#commit-guidelines)

## ⭐ تاریخچه ستاره‌ها

<a href="https://star-history.com/#kent8192/reinhardt-web&Date">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date&theme=dark" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date" />
   <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date" width="600" />
 </picture>
</a>

## مجوز

این پروژه تحت مجوز [BSD 3-Clause License](../../LICENSE) منتشر شده است.

### اعتبار طرف سوم

این پروژه از موارد زیر الهام گرفته است:

- [Django](https://www.djangoproject.com/) (مجوز BSD 3-Clause)
- [Django REST Framework](https://www.django-rest-framework.org/) (مجوز BSD 3-Clause)
- [FastAPI](https://fastapi.tiangolo.com/) (مجوز MIT)
- [SQLAlchemy](https://www.sqlalchemy.org/) (مجوز MIT)

اعتبار کامل را در [THIRD-PARTY-NOTICES](../../THIRD-PARTY-NOTICES) ببینید.

**نکته:** این پروژه وابسته یا تأیید شده توسط Django Software Foundation، Encode OSS Ltd.، Sebastián Ramírez (نویسنده FastAPI) یا Michael Bayer (نویسنده SQLAlchemy) نیست.
