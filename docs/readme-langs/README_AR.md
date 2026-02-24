<div align="center">
  <img src="../../branding/logo.png" alt="Reinhardt Logo" width="200"/>

  <h1>Reinhardt</h1>

  <h3>🦀 إطار عمل متعدد البنى مع بطاريات مضمنة</h3>

  <p><strong>إطار عمل API متكامل وقابل للتركيب لـ Rust</strong></p>
  <p>ابنِ بـ<em>كامل</em> قوة فلسفة Django "البطاريات مضمنة"،<br/>
  أو ركّب <em>فقط</em> ما تحتاجه—اختيارك، طريقتك.</p>

🌐 [English](../../README.md) | [日本語](README_JA.md) | [简体中文](README_ZH_CN.md) | [繁體中文](README_ZH_TW.md) | [Русский](README_RU.md) | [Українська](README_UK.md) | [فارسی](README_FA.md) | **العربية**

[![Crates.io](https://img.shields.io/crates/v/reinhardt-web.svg)](https://crates.io/crates/reinhardt-web)
[![Documentation](https://docs.rs/reinhardt-web/badge.svg)](https://docs.rs/reinhardt-web)
[![License](https://img.shields.io/badge/license-BSD--3--Clause-blue.svg)](../../LICENSE)
[![codecov](https://codecov.io/gh/kent8192/reinhardt-web/graph/badge.svg)](https://codecov.io/gh/kent8192/reinhardt-web)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/kent8192/reinhardt-web)

</div>

---

## 📍 التنقل السريع

قد تبحث عن:

- 🚀 [البداية السريعة](#البداية-السريعة) - ابدأ في 5 دقائق
- 📦 [خيارات التثبيت](#التثبيت) - اختر نوعك: Micro أو Standard أو Full
- 📚 [دليل البدء](../GETTING_STARTED.md) - دروس خطوة بخطوة
- 🎛️ [أعلام الميزات](../FEATURE_FLAGS.md) - ضبط دقيق للبناء
- 📖 [وثائق API](https://docs.rs/reinhardt-web) - مرجع API الكامل
- 💬 [المجتمع والدعم](#الحصول-على-المساعدة) - احصل على مساعدة من المجتمع

## لماذا Reinhardt؟

**Polylithic = Poly (متعدد) + Lithic (كتل بناء)**
على عكس الأطر الأحادية التي تجبرك على استخدام كل شيء، يتيح لك Reinhardt تركيب مكدسك المثالي من مكونات مستقلة ومختبرة جيداً.

Reinhardt يجمع أفضل ما في ثلاثة عوالم:

| الإلهام            | ما اقتبسناه                                            | ما حسّناه                                           |
|--------------------|--------------------------------------------------------|------------------------------------------------------|
| 🐍 **Django**      | فلسفة البطاريات المضمنة، تصميم ORM، لوحة الإدارة        | أعلام الميزات للبناء القابل للتركيب، سلامة أنواع Rust |
| 🎯 **Django REST** | المسلسلات، ViewSets، الأذونات                          | التحقق في وقت الترجمة، تجريدات بدون تكلفة            |
| ⚡ **FastAPI**      | نظام DI، OpenAPI التلقائي                               | أداء Rust الأصلي، بدون عبء وقت التشغيل              |
| 🗄️ **SQLAlchemy** | أنماط QuerySet، معالجة العلاقات                         | منشئ استعلامات آمن النوع، التحقق في وقت الترجمة      |

**النتيجة**: إطار عمل مألوف لمطوري Python، ولكن مع أداء وضمانات سلامة Rust.

## ✨ الميزات الرئيسية

- **ORM آمن النوع** مع التحقق في وقت الترجمة (reinhardt-query)
- **مسلسلات قوية** مع التحقق التلقائي (serde + validator)
- **DI بأسلوب FastAPI** مع حقن التبعية الآمن النوع والتخزين المؤقت
- **ViewSets** للتطوير السريع لـ CRUD API
- **مصادقة متعددة** (JWT، Token، Session، Basic) مع سمات BaseUser/FullUser
- **لوحة إدارة** مع واجهة إدارة النماذج المولدة تلقائياً
- **أوامر الإدارة** للترحيل والملفات الثابتة والمزيد
- **دعم GraphQL و WebSocket** للتطبيقات الفورية
- **الترقيم، التصفية، تحديد المعدل** مدمج
- **الإشارات** للبنية المعتمدة على الأحداث

انظر القائمة الكاملة في [المكونات المتاحة](#المكونات-المتاحة) والأمثلة في [دليل البدء](../GETTING_STARTED.md).

## التثبيت

Reinhardt إطار عمل معياري. اختر نقطة البداية:

**ملاحظة حول تسمية الصناديق:**
صندوق Reinhardt الرئيسي منشور على crates.io باسم `reinhardt-web`، لكنك تستورده كـ `reinhardt` في كودك باستخدام سمة `package`.

### الافتراضي: كامل الميزات (البطاريات مضمنة) ⚠️ الافتراضي الجديد

كل الميزات بدون تهيئة:

```toml
[dependencies]
# يُستورد كـ 'reinhardt'، منشور كـ 'reinhardt-web'
# الافتراضي يُفعّل كل الميزات (الحزمة الكاملة)
reinhardt = { version = "0.1.0-alpha.1", package = "reinhardt-web" }
```

**يشمل:** Database، Auth، REST API، Admin، GraphQL، WebSockets، Cache، i18n، Mail، Sessions، Static Files، Storage

**الثنائي**: ~50+ ميجابايت | **الترجمة**: أبطأ، لكن كل شيء يعمل فوراً

ثم استخدم في الكود:
```rust
use reinhardt::prelude::*;
use reinhardt::{Request, Response, StatusCode};
```

### الخيار 1: الإعداد القياسي (متوازن)

لمعظم المشاريع التي لا تحتاج كل الميزات:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", package = "reinhardt-web", default-features = false, features = ["standard"] }
```

**يشمل:** Core، Database (PostgreSQL)، REST API، Auth، Middleware، Pages (واجهة WASM مع SSR)

**الثنائي**: ~20-30 ميجابايت | **الترجمة**: متوسطة

### الخيار 2: الخدمات المصغرة (الإعداد الأدنى)

خفيف وسريع، مثالي لـ APIs البسيطة:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", package = "reinhardt-web", default-features = false, features = ["minimal"] }
```

**يشمل:** HTTP، التوجيه، DI، استخراج المعاملات، الخادم

**الثنائي**: ~5-10 ميجابايت | **الترجمة**: سريعة جداً

### الخيار 3: ابنِ مكدسك المخصص

ثبّت فقط المكونات المطلوبة:

```toml
[dependencies]
# المكونات الأساسية
reinhardt-http = "0.1.0-alpha.1"
reinhardt-urls = "0.1.0-alpha.1"

# اختياري: قاعدة البيانات
reinhardt-db = "0.1.0-alpha.1"

# اختياري: المصادقة
reinhardt-auth = "0.1.0-alpha.1"

# اختياري: ميزات REST API
reinhardt-rest = "0.1.0-alpha.1"

# اختياري: لوحة الإدارة
reinhardt-admin = "0.1.0-alpha.1"

# اختياري: الميزات المتقدمة
reinhardt-graphql = "0.1.0-alpha.1"
reinhardt-websockets = "0.1.0-alpha.1"
```

**📖 للقائمة الكاملة للصناديق وأعلام الميزات المتاحة، انظر [دليل أعلام الميزات](../FEATURE_FLAGS.md).**

## البداية السريعة

### 1. ثبّت أداة Reinhardt Admin

```bash
cargo install reinhardt-admin-cli
```

### 2. أنشئ مشروعاً جديداً

```bash
# إنشاء مشروع RESTful API (الافتراضي)
reinhardt-admin startproject my-api
cd my-api
```

هذا يُولّد هيكل المشروع الكامل:

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

**البديل: إنشاء مشروع reinhardt-pages (WASM + SSR)**

لواجهة WASM حديثة مع SSR:

```bash
# إنشاء مشروع pages
reinhardt-admin startproject my-app --with-pages
cd my-app

# تثبيت أدوات بناء WASM (المرة الأولى فقط)
cargo make install-wasm-tools

# بناء WASM وتشغيل خادم التطوير
cargo make dev
# زُر http://127.0.0.1:8000/
```

### 3. شغّل خادم التطوير

```bash
# باستخدام أمر manage
cargo run --bin manage runserver

# الخادم سيبدأ على http://127.0.0.1:8000
```

**دعم إعادة التحميل التلقائي:**

لإعادة التحميل التلقائي عند تغيير الكود (يتطلب bacon):

```bash
# تثبيت bacon
cargo install --locked bacon

# التشغيل مع إعادة التحميل التلقائي
bacon runserver

# أو استخدم cargo make
cargo make watch

# للاختبارات
bacon test
```

### 4. أنشئ تطبيقك الأول

```bash
# إنشاء تطبيق RESTful API (الافتراضي)
cargo run --bin manage startapp users

# أو حدد النوع صراحةً
cargo run --bin manage startapp users --restful

# إنشاء تطبيق Pages (WASM + SSR)
cargo run --bin manage startapp dashboard --with-pages
```

هذا يُنشئ هيكل التطبيق:

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

### 5. سجّل المسارات

عدّل `urls.rs` لتطبيقك:

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

ضمّنه في `src/config/urls.rs`:

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

ماكرو السمة `#[routes]` يسجّل هذه الدالة تلقائياً مع الإطار للاكتشاف عبر صندوق `inventory`.

**ملاحظة:** `reinhardt::prelude` يتضمن الأنواع شائعة الاستخدام. التصديرات الرئيسية:

**متاحة دائماً:**
- التوجيه والعروض الأساسية: `Router`، `DefaultRouter`، `ServerRouter`، `View`، `ListView`، `DetailView`
- ViewSets: `ViewSet`، `ModelViewSet`، `ReadOnlyModelViewSet`
- HTTP: `StatusCode`

**معتمدة على الميزات:**
- **ميزة `core`**: `Request`، `Response`، `Handler`، `Middleware`، الإشارات (`post_save`، `pre_save`، إلخ)
- **ميزة `database`**: `Model`، `DatabaseConnection`، `F`، `Q`، `Transaction`، `atomic`، دوال قاعدة البيانات (`Concat`، `Upper`، `Lower`، `Now`، `CurrentDate`)، دوال النوافذ (`Window`، `RowNumber`، `Rank`، `DenseRank`)، القيود (`UniqueConstraint`، `CheckConstraint`، `ForeignKeyConstraint`)
- **ميزة `auth`**: `User`، `UserManager`، `GroupManager`، `Permission`، `ObjectPermission`
- **ميزات `minimal` أو `standard` أو `di`**: `Body`، `Cookie`، `Header`، `Json`، `Path`، `Query`
- **ميزة `rest`**: المسلسلات، المحللات، الترقيم، التحكم بالمعدل، إصدار النسخ
- **ميزة `admin`**: مكونات لوحة الإدارة
- **ميزة `cache`**: `Cache`، `InMemoryCache`
- **ميزة `sessions`**: `Session`، `AuthenticationMiddleware`

انظر القائمة الكاملة في [دليل أعلام الميزات](../FEATURE_FLAGS.md).

للدليل الكامل خطوة بخطوة، انظر [دليل البدء](../GETTING_STARTED.md).

## 🎓 تعلم بالأمثلة

### مع قاعدة البيانات

هيّئ قاعدة البيانات في `settings/base.toml`:

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

الإعدادات تُحمّل تلقائياً في `src/config/settings.rs`:

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

**مصادر متغيرات البيئة:**

Reinhardt يوفر نوعين من مصادر متغيرات البيئة بأولويات مختلفة:

- **`EnvSource`** (الأولوية: 100) - متغيرات بيئة عالية الأولوية تتجاوز ملفات TOML
  ```rust
  .add_source(EnvSource::new().with_prefix("REINHARDT_"))
  ```

- **`LowPriorityEnvSource`** (الأولوية: 40) - متغيرات بيئة منخفضة الأولوية تعود لملفات TOML
  ```rust
  .add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
  ```

**ترتيب الأولوية**:
- مع `EnvSource`: متغيرات البيئة > `{profile}.toml` > `base.toml` > الافتراضيات
- مع `LowPriorityEnvSource` (موضح أعلاه): `{profile}.toml` > `base.toml` > متغيرات البيئة > الافتراضيات

اختر `EnvSource` عندما يجب أن تكون متغيرات البيئة دائماً لها الأولوية (مثل نشر الإنتاج).
اختر `LowPriorityEnvSource` عندما يجب أن تكون ملفات TOML المصدر الرئيسي للتهيئة (مثل التطوير).

انظر [وثائق الإعدادات](../SETTINGS_DOCUMENT.md) للتفاصيل.

**استخدام DefaultUser المدمج:**

Reinhardt يوفر تنفيذ `DefaultUser` جاهز للاستخدام (يتطلب ميزة `argon2-hasher`):

```rust
// users/models.rs
use reinhardt::prelude::*;
use reinhardt::DefaultUser;

// أعد تصدير DefaultUser كـ User لتطبيقك
pub type User = DefaultUser;

// DefaultUser يتضمن:
// - id: Uuid (المفتاح الأساسي)
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

// DefaultUser ينفذ:
// - سمة BaseUser (طرق المصادقة)
// - سمة FullUser (معلومات المستخدم الكاملة)
// - سمة PermissionsMixin (إدارة الأذونات)
// - سمة Model (عمليات قاعدة البيانات)
```

**تعريف نماذج مستخدم مخصصة:**

إذا كنت بحاجة لحقول مخصصة، عرّف نموذجك الخاص:

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

	// إضافة حقول مخصصة
	#[field(max_length = 50, null = true)]
	pub phone_number: Option<String>,
}
```

**ماكرو سمة Model:**

سمة `#[model(...)]` تولد تلقائياً:
- تنفيذ سمة `Model` (يتضمن وظائف `#[derive(Model)]`)
- موصلات حقول آمنة النوع: `User::field_email()`، `User::field_username()`، إلخ
- التسجيل في سجل النماذج العام
- دعم المفاتيح الأساسية المركبة

**ملاحظة:** عند استخدام `#[model(...)]`، لا تحتاج لإضافة `#[derive(Model)]` بشكل منفصل،
حيث يُطبق تلقائياً بواسطة سمة `#[model(...)]`.

**سمات الحقول:**
- `#[field(primary_key = true)]` - وضع علامة كمفتاح أساسي
- `#[field(max_length = 255)]` - تعيين الحد الأقصى للطول لحقول النصوص
- `#[field(default = value)]` - تعيين قيمة افتراضية
- `#[field(auto_now_add = true)]` - ملء timestamp تلقائياً عند الإنشاء
- `#[field(auto_now = true)]` - تحديث timestamp تلقائياً عند الحفظ
- `#[field(null = true)]` - السماح بقيم NULL
- `#[field(unique = true)]` - فرض قيد التفرد

للقائمة الكاملة لسمات الحقول، انظر [دليل سمات الحقول](../field_attributes.md).

موصلات الحقول المولدة تمكن الإشارة الآمنة للحقول في الاستعلامات:

```rust
// مولد بواسطة #[model(...)] لـ DefaultUser
impl DefaultUser {
	pub const fn field_id() -> FieldRef<DefaultUser, Uuid> { FieldRef::new("id") }
	pub const fn field_username() -> FieldRef<DefaultUser, String> { FieldRef::new("username") }
	pub const fn field_email() -> FieldRef<DefaultUser, String> { FieldRef::new("email") }
	pub const fn field_is_active() -> FieldRef<DefaultUser, bool> { FieldRef::new("is_active") }
	pub const fn field_is_staff() -> FieldRef<DefaultUser, bool> { FieldRef::new("is_staff") }
	pub const fn field_date_joined() -> FieldRef<DefaultUser, DateTime<Utc>> { FieldRef::new("date_joined") }
	// ... حقول أخرى
}
```

**أمثلة استعلامات متقدمة:**

```rust
use reinhardt::prelude::*;
use reinhardt::DefaultUser;

// استعلامات كائنات F/Q بأسلوب Django مع إشارات حقول آمنة النوع
async fn complex_user_query() -> Result<Vec<DefaultUser>, Box<dyn std::error::Error>> {
	// كائنات Q مع إشارات حقول آمنة النوع (باستخدام موصلات الحقول المولدة)
	let active_query = Q::new()
		.field("is_active").eq(true)
		.and(Q::new().field("date_joined").gte(Now::new()));

	// دوال قاعدة البيانات مع إشارات حقول آمنة النوع
	let email_lower = Lower::new(DefaultUser::field_email().into());
	let username_upper = Upper::new(DefaultUser::field_username().into());

	// التجميعات باستخدام موصلات الحقول
	let user_count = Aggregate::count(DefaultUser::field_id().into());
	let latest_joined = Aggregate::max(DefaultUser::field_date_joined().into());

	// دوال النوافذ للترتيب
	let rank_by_join_date = Window::new()
		.partition_by(vec![DefaultUser::field_is_active().into()])
		.order_by(vec![(DefaultUser::field_date_joined().into(), "DESC")])
		.function(RowNumber::new());

	todo!("Execute query with these components")
}

// دعم المعاملات
async fn create_user_with_transaction(
	conn: &DatabaseConnection,
	user_data: CreateUserRequest
) -> Result<User, Box<dyn std::error::Error>> {
	// معاملة مع تراجع تلقائي عند الخطأ
	transaction(conn, |_tx| async move {
		let user = User::create(user_data).await?;
		log_user_creation(&user).await?;
		Ok(user)
	}).await
}
```

**ملاحظة**: Reinhardt يستخدم reinhardt-query لعمليات SQL. ماكرو `#[derive(Model)]` يولد تلقائياً تنفيذات سمة Model، وموصلات حقول آمنة النوع، والتسجيل في سجل النماذج العام.

سجّل في `src/config/apps.rs`:

```rust
// src/config/apps.rs
use reinhardt::installed_apps;

// ماكرو installed_apps! يولد:
// - enum InstalledApp مع متغيرات لكل تطبيق
// - تنفيذ سمات التحويل (From، Into، Display)
// - سجل لتهيئة واكتشاف التطبيقات
//
// ملاحظة: على عكس INSTALLED_APPS في Django، هذا الماكرو لتطبيقات المستخدم فقط.
// ميزات الإطار المدمجة (auth، sessions، admin، إلخ) تُفعّل عبر
// أعلام ميزات Cargo، وليس عبر installed_apps!.
//
// مثال:
// [dependencies]
// reinhardt = { version = "0.1", features = ["auth", "sessions", "admin"] }
//
// هذا يُفعّل:
// - اكتشاف التطبيقات التلقائي للترحيلات، لوحة الإدارة، إلخ
// - إشارات تطبيقات آمنة النوع في كودك
// - تهيئة تطبيقات مركزية
installed_apps! {
	users: "users",
}

pub fn get_installed_apps() -> Vec<String> {
	InstalledApp::all_apps()
}
```

### مع المصادقة

Reinhardt يوفر نماذج مستخدمين بأسلوب Django مع سمات `BaseUser` و `FullUser`، بالإضافة إلى إدارة مستخدمين شاملة عبر `UserManager`.

**ملاحظة:** Reinhardt يتضمن تنفيذ `DefaultUser` مدمج. يمكنك استخدامه مباشرة أو تعريف نموذج المستخدم الخاص بك كما هو موضح أدناه.

**مثال إدارة المستخدمين:**

```rust
use reinhardt::prelude::*;

// إنشاء وإدارة المستخدمين مع UserManager
async fn manage_users() -> Result<(), Box<dyn std::error::Error>> {
	let hasher = Argon2Hasher::new();
	let user_manager = UserManager::new(hasher);

	// إنشاء مستخدم جديد
	let user = user_manager.create_user(CreateUserData {
		username: "alice".to_string(),
		email: "alice@example.com".to_string(),
		password: "secure_password".to_string(),
		first_name: Some("Alice".to_string()),
		last_name: Some("Smith".to_string()),
	}).await?;

	// تحديث معلومات المستخدم
	user_manager.update_user(user.id, UpdateUserData {
		email: Some("alice.smith@example.com".to_string()),
		is_active: Some(true),
		..Default::default()
	}).await?;

	// إدارة المجموعات والأذونات
	let group_manager = GroupManager::new();
	let editors = group_manager.create_group(CreateGroupData {
		name: "editors".to_string(),
	}).await?;

	// تعيين أذونات على مستوى الكائن
	let permission = ObjectPermission::new("edit", user.id, article.id);
	let perm_checker = ObjectPermissionChecker::new();
	if perm_checker.has_permission(&user, "edit", &article).await? {
		// المستخدم يمكنه تحرير المقالة
	}

	Ok(())
}
```

استخدم `DefaultUser` المدمج في `users/models.rs`:

```rust
// users/models.rs
use reinhardt::DefaultUser;

// أعد تصدير DefaultUser كنوع User الخاص بك
pub type User = DefaultUser;

// DefaultUser ينفذ بالفعل:
// - سمة BaseUser (طرق المصادقة)
// - سمة FullUser (username، email، first_name، last_name، إلخ)
// - سمة PermissionsMixin (إدارة الأذونات)
// - سمة Model (عمليات قاعدة البيانات)
```

**لنماذج المستخدم المخصصة:**

إذا كنت بحاجة لحقول إضافية تتجاوز DefaultUser، عرّف الخاص بك:

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

	// حقول مخصصة
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

استخدم مصادقة JWT في `views/profile.rs` لتطبيقك:

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
	// استخراج رمز JWT من رأس Authorization
	let auth_header = req.headers.get("authorization")
		.and_then(|h| h.to_str().ok())
		.ok_or("Missing Authorization header")?;

	let token = auth_header.strip_prefix("Bearer ")
		.ok_or("Invalid Authorization header format")?;

	// التحقق من الرمز والحصول على معرف المستخدم
	let jwt_auth = JwtAuth::new(b"your-secret-key");
	let claims = jwt_auth.verify_token(token)?;

	// تحميل المستخدم من قاعدة البيانات باستخدام claims.user_id
	let user = User::find_by_id(&db, &claims.user_id).await?;

	// التحقق من أن المستخدم نشط
	if !user.is_active() {
		return Err("User account is inactive".into());
	}

	// إرجاع ملف المستخدم كـ JSON
	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

### تعريف نقاط النهاية

Reinhardt يستخدم مزينات طرق HTTP لتعريف نقاط النهاية:

#### مزينات طرق HTTP

استخدم `#[get]`، `#[post]`، `#[put]`، `#[delete]` لتعريف المسارات:

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

**الميزات:**
- التحقق من المسار في وقت الترجمة
- بناء جملة موجز
- ربط طريقة HTTP تلقائي
- دعم حقن التبعية عبر `#[inject]`

#### استخدام حقن التبعية

ادمج مزينات طرق HTTP مع `#[inject]` لحقن التبعية التلقائي:

```rust
use reinhardt::{get, Request, Response, StatusCode, ViewResult};
use reinhardt::db::DatabaseConnection;
use std::sync::Arc;

#[get("/users/{id}/", name = "get_user")]
pub async fn get_user(
	req: Request,
	#[inject] db: Arc<DatabaseConnection>,  // يُحقن تلقائياً
) -> ViewResult<Response> {
	let id = req.path_params.get("id")
		.ok_or("Missing id")?
		.parse::<i64>()?;

	// استخدام اتصال قاعدة البيانات المحقون
	let user = db.query("SELECT * FROM users WHERE id = $1")
		.bind(id)
		.fetch_one()
		.await?;

	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

**ميزات حقن التبعية:**
- حقن التبعية التلقائي عبر سمة `#[inject]`
- التحكم في التخزين المؤقت مع `#[inject(cache = false)]`
- نظام حقن تبعية مستوحى من FastAPI
- يعمل بسلاسة مع مزينات طرق HTTP

**نوع الإرجاع:**

كل دوال العرض تستخدم `ViewResult<T>` كنوع إرجاع:

```rust
use reinhardt::ViewResult;  // نوع نتيجة معرف مسبقاً
```

### مع استخراج المعاملات

في `views/user.rs` لتطبيقك:

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
	// استخراج معامل المسار من الطلب
	let id = req.path_params.get("id")
		.ok_or("Missing id parameter")?
		.parse::<i64>()
		.map_err(|_| "Invalid id format")?;

	// استخراج معاملات الاستعلام (مثلاً ?include_inactive=true)
	let include_inactive = req.query_params.get("include_inactive")
		.and_then(|v| v.parse::<bool>().ok())
		.unwrap_or(false);

	// جلب المستخدم من قاعدة البيانات باستخدام الاتصال المحقون
	let user = User::find_by_id(&db, id).await?;

	// التحقق من حالة النشاط إذا لزم الأمر
	if !include_inactive && !user.is_active {
		return Err("User is inactive".into());
	}

	// إرجاع كـ JSON
	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

سجّل المسار مع معامل المسار في `urls.rs`:

```rust
// users/urls.rs
use reinhardt::ServerRouter;

use super::views;

pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::get_user)  // المسار معرف في #[get("/users/{id}/")]
}
```

### مع المسلسلات والتحقق

في `serializers/user.rs` لتطبيقك:

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

في `views/user.rs` لتطبيقك:

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
	// تحليل جسم الطلب
	let body_bytes = std::mem::take(&mut req.body);
	let create_req: CreateUserRequest = serde_json::from_slice(&body_bytes)?;

	// التحقق من الطلب
	create_req.validate()?;

	// إنشاء المستخدم
	let mut user = User {
		id: 0, // سيُعيّن بواسطة قاعدة البيانات
		username: create_req.username,
		email: create_req.email,
		password_hash: None,
		is_active: true,
		created_at: Utc::now(),
	};

	// تجزئة كلمة المرور باستخدام سمة BaseUser
	user.set_password(&create_req.password)?;

	// الحفظ في قاعدة البيانات باستخدام الاتصال المحقون
	user.save(&db).await?;

	// التحويل للاستجابة
	let response_data = UserResponse::from(user);
	let json = serde_json::to_string(&response_data)?;

	Ok(Response::new(StatusCode::CREATED)
		.with_body(json))
}
```

## المكونات المتاحة

Reinhardt يقدم مكونات معيارية قابلة للمزج:

| المكون              | اسم الصندوق               | الميزات                                     |
|---------------------|---------------------------|---------------------------------------------|
| **النواة**           |                           |                                             |
| الأنواع الأساسية     | `reinhardt-core`          | السمات والأنواع والماكرو الأساسية (Model، endpoint)|
| HTTP والتوجيه       | `reinhardt-http`          | Request/Response، معالجة HTTP               |
| توجيه URL           | `reinhardt-urls`          | مسارات قائمة على الدوال والفئات            |
| الخادم              | `reinhardt-server`        | تنفيذ خادم HTTP                             |
| Middleware          | `reinhardt-dispatch`      | سلسلة Middleware، إرسال الإشارات           |
| التهيئة             | `reinhardt-conf`          | إدارة الإعدادات، تحميل البيئة              |
| الأوامر             | `reinhardt-commands`      | أدوات CLI للإدارة (startproject، إلخ)      |
| الاختصارات          | `reinhardt-shortcuts`     | دوال مساعدة شائعة                          |
| **قاعدة البيانات**   |                           |                                             |
| ORM                 | `reinhardt-db`            | تكامل reinhardt-query                      |
| **المصادقة**        |                           |                                             |
| Auth                | `reinhardt-auth`          | JWT، Token، Session، Basic auth، نماذج User|
| **REST API**        |                           |                                             |
| المسلسلات           | `reinhardt-rest`          | تكامل serde/validator، ViewSets             |
| **النماذج**          |                           |                                             |
| النماذج             | `reinhardt-forms`         | معالجة النماذج والتحقق                      |
| **متقدم**           |                           |                                             |
| لوحة الإدارة        | `reinhardt-admin`         | واجهة إدارة بأسلوب Django                   |
| نظام الإضافات       | `reinhardt-dentdelion`    | دعم إضافات ثابتة و WASM، إدارة CLI         |
| المهام الخلفية       | `reinhardt-tasks`         | طوابير المهام (Redis، RabbitMQ، SQLite)    |
| GraphQL             | `reinhardt-graphql`       | توليد المخطط، الاشتراكات                    |
| WebSockets          | `reinhardt-websockets`    | الاتصال الفوري                              |
| i18n                | `reinhardt-i18n`          | دعم تعدد اللغات                             |
| **الاختبار**         |                           |                                             |
| أدوات الاختبار      | `reinhardt-test`          | مساعدات الاختبار، التثبيتات، TestContainers |

**لأعلام الميزات التفصيلية في كل صندوق، انظر [دليل أعلام الميزات](../FEATURE_FLAGS.md).**

---

## الوثائق

- 📚 [دليل البدء](../GETTING_STARTED.md) - دروس خطوة بخطوة للمبتدئين
- 🎛️ [دليل أعلام الميزات](../FEATURE_FLAGS.md) - تحسين البناء بالتحكم الدقيق بالميزات
- 📖 [مرجع API](https://docs.rs/reinhardt) (قريباً)
- 📝 [الدروس التعليمية](../tutorials/) - تعلم ببناء تطبيقات حقيقية

**لمساعدي AI**: انظر [CLAUDE.md](../../CLAUDE.md) لمعايير البرمجة الخاصة بالمشروع وإرشادات الاختبار واتفاقيات التطوير.

## 💬 الحصول على المساعدة

Reinhardt مشروع يقوده المجتمع. إليك أين تحصل على المساعدة:

- 💬 **Discord**: انضم إلى خادم Discord للدردشة الفورية (قريباً)
- 💭 **GitHub Discussions**: [اطرح أسئلة وشارك الأفكار](https://github.com/kent8192/reinhardt-web/discussions)
- 🐛 **Issues**: [أبلغ عن الأخطاء](https://github.com/kent8192/reinhardt-web/issues)
- 📖 **الوثائق**: [اقرأ الأدلة](../)

قبل السؤال، يرجى التحقق من:

- ✅ [دليل البدء](../GETTING_STARTED.md)
- ✅ [الأمثلة](../../examples/)
- ✅ Issues و Discussions الموجودة على GitHub

## 🤝 المساهمة

نحب المساهمات! يرجى قراءة [دليل المساهمة](../../CONTRIBUTING.md) للبدء.

**روابط سريعة**:

- [إعداد التطوير](../../CONTRIBUTING.md#development-setup)
- [إرشادات الاختبار](../../CONTRIBUTING.md#testing-guidelines)
- [إرشادات الإيداع](../../CONTRIBUTING.md#commit-guidelines)

## ⭐ تاريخ النجوم

<a href="https://star-history.com/#kent8192/reinhardt-web&Date">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date&theme=dark" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date" />
   <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date" width="600" />
 </picture>
</a>

## الترخيص

هذا المشروع مرخص بموجب [BSD 3-Clause License](../../LICENSE).

### إسناد الطرف الثالث

هذا المشروع مستوحى من:

- [Django](https://www.djangoproject.com/) (ترخيص BSD 3-Clause)
- [Django REST Framework](https://www.django-rest-framework.org/) (ترخيص BSD 3-Clause)
- [FastAPI](https://fastapi.tiangolo.com/) (ترخيص MIT)
- [SQLAlchemy](https://www.sqlalchemy.org/) (ترخيص MIT)

انظر الإسناد الكامل في [THIRD-PARTY-NOTICES](../../THIRD-PARTY-NOTICES).

**ملاحظة:** هذا المشروع غير تابع أو معتمد من Django Software Foundation أو Encode OSS Ltd. أو Sebastián Ramírez (مؤلف FastAPI) أو Michael Bayer (مؤلف SQLAlchemy).
