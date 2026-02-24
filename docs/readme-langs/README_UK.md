<div align="center">
  <img src="../../branding/logo.png" alt="Reinhardt Logo" width="200"/>

  <h1>Reinhardt</h1>

  <h3>🦀 Полілітичний фреймворк з батарейками</h3>

  <p><strong>Компонований повнофункціональний API-фреймворк для Rust</strong></p>
  <p>Будуйте з <em>усією</em> потужністю філософії Django "батарейки в комплекті",<br/>
  або компонуйте <em>тільки</em> те, що вам потрібно — ваш вибір, ваш шлях.</p>

🌐 [English](../../README.md) | [日本語](README_JA.md) | [简体中文](README_ZH_CN.md) | [繁體中文](README_ZH_TW.md) | [Русский](README_RU.md) | **Українська** | [فارسی](README_FA.md) | [العربية](README_AR.md)

[![Crates.io](https://img.shields.io/crates/v/reinhardt-web.svg)](https://crates.io/crates/reinhardt-web)
[![Documentation](https://docs.rs/reinhardt-web/badge.svg)](https://docs.rs/reinhardt-web)
[![License](https://img.shields.io/badge/license-BSD--3--Clause-blue.svg)](../../LICENSE)
[![codecov](https://codecov.io/gh/kent8192/reinhardt-web/graph/badge.svg)](https://codecov.io/gh/kent8192/reinhardt-web)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/kent8192/reinhardt-web)

</div>

---

## 📍 Швидка навігація

Можливо, ви шукаєте:

- 🚀 [Швидкий старт](#швидкий-старт) - Запуск за 5 хвилин
- 📦 [Варіанти встановлення](#встановлення) - Оберіть свій варіант: Micro, Standard або Full
- 📚 [Посібник початківця](../GETTING_STARTED.md) - Покрокове керівництво
- 🎛️ [Прапорці функцій](../FEATURE_FLAGS.md) - Тонке налаштування збірки
- 📖 [API документація](https://docs.rs/reinhardt-web) - Повний довідник API
- 💬 [Спільнота та підтримка](#отримання-допомоги) - Отримайте допомогу від спільноти

## Чому Reinhardt?

**Polylithic = Poly (багато) + Lithic (будівельні блоки)**
На відміну від монолітних фреймворків, які змушують вас використовувати все, Reinhardt дозволяє компонувати ідеальний стек з незалежних, добре протестованих компонентів.

Reinhardt об'єднує найкраще з трьох світів:

| Натхнення          | Що ми запозичили                                       | Що ми покращили                                     |
|--------------------|--------------------------------------------------------|------------------------------------------------------|
| 🐍 **Django**      | Філософія "батарейки в комплекті", дизайн ORM, адмінка | Прапорці функцій для компонованих збірок, типобезпека Rust |
| 🎯 **Django REST** | Серіалізатори, ViewSets, дозволи                       | Перевірка під час компіляції, абстракції з нульовою вартістю |
| ⚡ **FastAPI**      | DI система, автоматичний OpenAPI                        | Нативна продуктивність Rust, без накладних витрат під час виконання |
| 🗄️ **SQLAlchemy** | Патерни QuerySet, обробка зв'язків                      | Типобезпечний конструктор запитів, перевірка під час компіляції |

**Результат**: Фреймворк, знайомий Python-розробникам, але з продуктивністю та гарантіями безпеки Rust.

## ✨ Ключові функції

- **Типобезпечна ORM** з перевіркою під час компіляції (reinhardt-query)
- **Потужні серіалізатори** з автоматичною валідацією (serde + validator)
- **DI у стилі FastAPI** з типобезпечним впровадженням залежностей та кешуванням
- **ViewSets** для швидкої розробки CRUD API
- **Множинна автентифікація** (JWT, Token, Session, Basic) з трейтами BaseUser/FullUser
- **Адмін-панель** з автоматично генерованим інтерфейсом управління моделями
- **Команди управління** для міграцій, статичних файлів тощо
- **GraphQL та WebSocket** підтримка для застосунків реального часу
- **Пагінація, фільтрація, обмеження швидкості** вбудовані
- **Сигнали** для подієво-орієнтованої архітектури

Повний список див. у [Доступні компоненти](#доступні-компоненти), приклади у [Посібнику початківця](../GETTING_STARTED.md).

## Встановлення

Reinhardt — модульний фреймворк. Оберіть точку старту:

**Примітка щодо назви крейтів:**
Основний крейт Reinhardt опублікований на crates.io як `reinhardt-web`, але ви імпортуєте його як `reinhardt` у коді, використовуючи атрибут `package`.

### За замовчуванням: Повнофункціональний (Батарейки в комплекті) ⚠️ Новий default

Усі функції без налаштування:

```toml
[dependencies]
# Імпортується як 'reinhardt', опублікований як 'reinhardt-web'
# За замовчуванням увімкнені ВСІ функції (повний комплект)
reinhardt = { version = "0.1.0-alpha.1", package = "reinhardt-web" }
```

**Включає:** Database, Auth, REST API, Admin, GraphQL, WebSockets, Cache, i18n, Mail, Sessions, Static Files, Storage

**Бінарник**: ~50+ МБ | **Компіляція**: Повільніше, але все працює з коробки

Потім використовуйте в коді:
```rust
use reinhardt::prelude::*;
use reinhardt::{Request, Response, StatusCode};
```

### Варіант 1: Стандартне встановлення (Збалансований)

Для більшості проєктів, яким не потрібні всі функції:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", package = "reinhardt-web", default-features = false, features = ["standard"] }
```

**Включає:** Core, Database (PostgreSQL), REST API, Auth, Middleware, Pages (WASM фронтенд з SSR)

**Бінарник**: ~20-30 МБ | **Компіляція**: Середня

### Варіант 2: Мікросервіси (Мінімальне встановлення)

Легкий та швидкий, ідеальний для простих API:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", package = "reinhardt-web", default-features = false, features = ["minimal"] }
```

**Включає:** HTTP, маршрутизація, DI, вилучення параметрів, сервер

**Бінарник**: ~5-10 МБ | **Компіляція**: Дуже швидка

### Варіант 3: Створіть свій стек

Встановлюйте лише потрібні компоненти:

```toml
[dependencies]
# Основні компоненти
reinhardt-http = "0.1.0-alpha.1"
reinhardt-urls = "0.1.0-alpha.1"

# Опціонально: База даних
reinhardt-db = "0.1.0-alpha.1"

# Опціонально: Автентифікація
reinhardt-auth = "0.1.0-alpha.1"

# Опціонально: REST API функції
reinhardt-rest = "0.1.0-alpha.1"

# Опціонально: Адмін-панель
reinhardt-admin = "0.1.0-alpha.1"

# Опціонально: Розширені функції
reinhardt-graphql = "0.1.0-alpha.1"
reinhardt-websockets = "0.1.0-alpha.1"
```

**📖 Повний список доступних крейтів та прапорців функцій див. у [Посібнику з прапорців функцій](../FEATURE_FLAGS.md).**

## Швидкий старт

### 1. Встановіть Reinhardt Admin Tool

```bash
cargo install reinhardt-admin-cli
```

### 2. Створіть новий проєкт

```bash
# Створення RESTful API проєкту (за замовчуванням)
reinhardt-admin startproject my-api
cd my-api
```

Це створить повну структуру проєкту:

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

**Альтернатива: Створення reinhardt-pages проєкту (WASM + SSR)**

Для сучасного WASM-фронтенду з SSR:

```bash
# Створення pages проєкту
reinhardt-admin startproject my-app --with-pages
cd my-app

# Встановлення WASM інструментів збірки (тільки перший раз)
cargo make install-wasm-tools

# Збірка WASM та запуск сервера розробки
cargo make dev
# Відкрийте http://127.0.0.1:8000/
```

### 3. Запустіть сервер розробки

```bash
# Використовуючи команду manage
cargo run --bin manage runserver

# Сервер запуститься на http://127.0.0.1:8000
```

**Підтримка автоперезавантаження:**

Для автоматичного перезавантаження при зміні коду (потрібен bacon):

```bash
# Встановлення bacon
cargo install --locked bacon

# Запуск з автоперезавантаженням
bacon runserver

# Або використовуйте cargo make
cargo make watch

# Для тестів
bacon test
```

### 4. Створіть перший застосунок

```bash
# Створення RESTful API застосунку (за замовчуванням)
cargo run --bin manage startapp users

# Або явно вкажіть тип
cargo run --bin manage startapp users --restful

# Створення Pages застосунку (WASM + SSR)
cargo run --bin manage startapp dashboard --with-pages
```

Це створить структуру застосунку:

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

### 5. Зареєструйте маршрути

Відредагуйте `urls.rs` вашого застосунку:

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

Включіть у `src/config/urls.rs`:

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

Атрибутний макрос `#[routes]` автоматично реєструє цю функцію у фреймворку для виявлення через крейт `inventory`.

**Примітка:** `reinhardt::prelude` включає часто використовувані типи. Основні експорти:

**Завжди доступні:**
- Базова маршрутизація та представлення: `Router`, `DefaultRouter`, `ServerRouter`, `View`, `ListView`, `DetailView`
- ViewSets: `ViewSet`, `ModelViewSet`, `ReadOnlyModelViewSet`
- HTTP: `StatusCode`

**Залежать від функцій:**
- **Функція `core`**: `Request`, `Response`, `Handler`, `Middleware`, Сигнали (`post_save`, `pre_save` та ін.)
- **Функція `database`**: `Model`, `DatabaseConnection`, `F`, `Q`, `Transaction`, `atomic`, Функції БД (`Concat`, `Upper`, `Lower`, `Now`, `CurrentDate`), Віконні функції (`Window`, `RowNumber`, `Rank`, `DenseRank`), Обмеження (`UniqueConstraint`, `CheckConstraint`, `ForeignKeyConstraint`)
- **Функція `auth`**: `User`, `UserManager`, `GroupManager`, `Permission`, `ObjectPermission`
- **Функції `minimal`, `standard` або `di`**: `Body`, `Cookie`, `Header`, `Json`, `Path`, `Query`
- **Функція `rest`**: Серіалізатори, Парсери, Пагінація, Тротлінг, Версіонування
- **Функція `admin`**: Компоненти адмін-панелі
- **Функція `cache`**: `Cache`, `InMemoryCache`
- **Функція `sessions`**: `Session`, `AuthenticationMiddleware`

Повний список див. у [Посібнику з прапорців функцій](../FEATURE_FLAGS.md).

Повне покрокове керівництво див. у [Посібнику початківця](../GETTING_STARTED.md).

## 🎓 Вчіться на прикладах

### З базою даних

Налаштуйте базу даних у `settings/base.toml`:

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

Налаштування автоматично завантажуються в `src/config/settings.rs`:

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

**Джерела змінних середовища:**

Reinhardt надає два типи джерел змінних середовища з різними пріоритетами:

- **`EnvSource`** (пріоритет: 100) - Високопріоритетні змінні середовища, які перевизначають TOML файли
  ```rust
  .add_source(EnvSource::new().with_prefix("REINHARDT_"))
  ```

- **`LowPriorityEnvSource`** (пріоритет: 40) - Низькопріоритетні змінні середовища, які використовуються як запасний варіант
  ```rust
  .add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
  ```

**Порядок пріоритетів**:
- З `EnvSource`: Змінні середовища > `{profile}.toml` > `base.toml` > Значення за замовчуванням
- З `LowPriorityEnvSource` (показано вище): `{profile}.toml` > `base.toml` > Змінні середовища > Значення за замовчуванням

Обирайте `EnvSource`, коли змінні середовища завжди повинні мати пріоритет (наприклад, production).
Обирайте `LowPriorityEnvSource`, коли TOML файли повинні бути основним джерелом конфігурації (наприклад, розробка).

Див. [Документацію з налаштувань](../SETTINGS_DOCUMENT.md) для деталей.

**Використання вбудованого DefaultUser:**

Reinhardt надає готову реалізацію `DefaultUser` (потрібна функція `argon2-hasher`):

```rust
// users/models.rs
use reinhardt::prelude::*;
use reinhardt::DefaultUser;

// Реекспортуйте DefaultUser як User для вашого застосунку
pub type User = DefaultUser;

// DefaultUser включає:
// - id: Uuid (первинний ключ)
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

// DefaultUser реалізує:
// - Трейт BaseUser (методи автентифікації)
// - Трейт FullUser (повна інформація про користувача)
// - Трейт PermissionsMixin (управління дозволами)
// - Трейт Model (операції з БД)
```

**Визначення користувацьких моделей:**

Якщо потрібні додаткові поля, визначте свою модель:

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

	// Додайте користувацькі поля
	#[field(max_length = 50, null = true)]
	pub phone_number: Option<String>,
}
```

**Атрибутний макрос Model:**

Атрибут `#[model(...)]` автоматично генерує:
- Реалізацію трейта `Model` (включає функціональність `#[derive(Model)]`)
- Типобезпечні аксесори полів: `User::field_email()`, `User::field_username()` та ін.
- Реєстрацію в глобальному реєстрі моделей
- Підтримку складених первинних ключів

**Примітка:** При використанні `#[model(...)]` НЕ потрібно додавати `#[derive(Model)]` окремо,
оскільки він автоматично застосовується атрибутом `#[model(...)]`.

**Атрибути полів:**
- `#[field(primary_key = true)]` - Позначити як первинний ключ
- `#[field(max_length = 255)]` - Встановити максимальну довжину для рядкових полів
- `#[field(default = value)]` - Встановити значення за замовчуванням
- `#[field(auto_now_add = true)]` - Автозаповнення timestamp при створенні
- `#[field(auto_now = true)]` - Автооновлення timestamp при збереженні
- `#[field(null = true)]` - Дозволити NULL значення
- `#[field(unique = true)]` - Застосувати обмеження унікальності

Повний список атрибутів полів див. у [Посібнику з атрибутів полів](../field_attributes.md).

Згенеровані аксесори полів дозволяють типобезпечно посилатися на поля в запитах:

```rust
// Згенеровано #[model(...)] для DefaultUser
impl DefaultUser {
	pub const fn field_id() -> FieldRef<DefaultUser, Uuid> { FieldRef::new("id") }
	pub const fn field_username() -> FieldRef<DefaultUser, String> { FieldRef::new("username") }
	pub const fn field_email() -> FieldRef<DefaultUser, String> { FieldRef::new("email") }
	pub const fn field_is_active() -> FieldRef<DefaultUser, bool> { FieldRef::new("is_active") }
	pub const fn field_is_staff() -> FieldRef<DefaultUser, bool> { FieldRef::new("is_staff") }
	pub const fn field_date_joined() -> FieldRef<DefaultUser, DateTime<Utc>> { FieldRef::new("date_joined") }
	// ... інші поля
}
```

**Приклади розширених запитів:**

```rust
use reinhardt::prelude::*;
use reinhardt::DefaultUser;

// Django-стиль F/Q об'єктних запитів з типобезпечними посиланнями на поля
async fn complex_user_query() -> Result<Vec<DefaultUser>, Box<dyn std::error::Error>> {
	// Q об'єкти з типобезпечними посиланнями на поля (використовуючи згенеровані аксесори)
	let active_query = Q::new()
		.field("is_active").eq(true)
		.and(Q::new().field("date_joined").gte(Now::new()));

	// Функції БД з типобезпечними посиланнями на поля
	let email_lower = Lower::new(DefaultUser::field_email().into());
	let username_upper = Upper::new(DefaultUser::field_username().into());

	// Агрегації використовуючи аксесори полів
	let user_count = Aggregate::count(DefaultUser::field_id().into());
	let latest_joined = Aggregate::max(DefaultUser::field_date_joined().into());

	// Віконні функції для ранжування
	let rank_by_join_date = Window::new()
		.partition_by(vec![DefaultUser::field_is_active().into()])
		.order_by(vec![(DefaultUser::field_date_joined().into(), "DESC")])
		.function(RowNumber::new());

	todo!("Execute query with these components")
}

// Підтримка транзакцій
async fn create_user_with_transaction(
	conn: &DatabaseConnection,
	user_data: CreateUserRequest
) -> Result<User, Box<dyn std::error::Error>> {
	// Транзакція з автоматичним відкатом при помилці
	transaction(conn, |_tx| async move {
		let user = User::create(user_data).await?;
		log_user_creation(&user).await?;
		Ok(user)
	}).await
}
```

**Примітка**: Reinhardt використовує reinhardt-query для SQL операцій. Макрос `#[derive(Model)]` автоматично генерує реалізації трейта Model, типобезпечні аксесори полів та реєстрацію в глобальному реєстрі моделей.

Зареєструйте в `src/config/apps.rs`:

```rust
// src/config/apps.rs
use reinhardt::installed_apps;

// Макрос installed_apps! генерує:
// - Enum InstalledApp з варіантами для кожного застосунку
// - Реалізацію конверсійних трейтів (From, Into, Display)
// - Реєстр для конфігурації та виявлення застосунків
//
// Примітка: На відміну від INSTALLED_APPS Django, цей макрос тільки для користувацьких застосунків.
// Вбудовані функції фреймворку (auth, sessions, admin та ін.) вмикаються через
// прапорці функцій Cargo, а не через installed_apps!.
//
// Приклад:
// [dependencies]
// reinhardt = { version = "0.1", features = ["auth", "sessions", "admin"] }
//
// Це вмикає:
// - Автоматичне виявлення застосунків для міграцій, адмін-панелі та ін.
// - Типобезпечні посилання на застосунки в коді
// - Централізовану конфігурацію застосунків
installed_apps! {
	users: "users",
}

pub fn get_installed_apps() -> Vec<String> {
	InstalledApp::all_apps()
}
```

### З автентифікацією

Reinhardt надає Django-стиль моделі користувачів з трейтами `BaseUser` та `FullUser`, а також комплексне управління користувачами через `UserManager`.

**Примітка:** Reinhardt включає вбудовану реалізацію `DefaultUser`. Ви можете використовувати її напряму або визначити свою модель користувача, як показано нижче.

**Приклад управління користувачами:**

```rust
use reinhardt::prelude::*;

// Створення та управління користувачами з UserManager
async fn manage_users() -> Result<(), Box<dyn std::error::Error>> {
	let hasher = Argon2Hasher::new();
	let user_manager = UserManager::new(hasher);

	// Створення нового користувача
	let user = user_manager.create_user(CreateUserData {
		username: "alice".to_string(),
		email: "alice@example.com".to_string(),
		password: "secure_password".to_string(),
		first_name: Some("Alice".to_string()),
		last_name: Some("Smith".to_string()),
	}).await?;

	// Оновлення інформації про користувача
	user_manager.update_user(user.id, UpdateUserData {
		email: Some("alice.smith@example.com".to_string()),
		is_active: Some(true),
		..Default::default()
	}).await?;

	// Управління групами та дозволами
	let group_manager = GroupManager::new();
	let editors = group_manager.create_group(CreateGroupData {
		name: "editors".to_string(),
	}).await?;

	// Призначення дозволів на рівні об'єктів
	let permission = ObjectPermission::new("edit", user.id, article.id);
	let perm_checker = ObjectPermissionChecker::new();
	if perm_checker.has_permission(&user, "edit", &article).await? {
		// Користувач може редагувати статтю
	}

	Ok(())
}
```

Використовуйте вбудований `DefaultUser` у `users/models.rs`:

```rust
// users/models.rs
use reinhardt::DefaultUser;

// Реекспортуйте DefaultUser як ваш тип User
pub type User = DefaultUser;

// DefaultUser вже реалізує:
// - Трейт BaseUser (методи автентифікації)
// - Трейт FullUser (username, email, first_name, last_name та ін.)
// - Трейт PermissionsMixin (управління дозволами)
// - Трейт Model (операції з БД)
```

**Для користувацьких моделей:**

Якщо потрібні додаткові поля крім DefaultUser, визначте свою:

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

	// Користувацькі поля
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

Використовуйте JWT автентифікацію у `views/profile.rs` вашого застосунку:

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
	// Вилучення JWT токена з заголовка Authorization
	let auth_header = req.headers.get("authorization")
		.and_then(|h| h.to_str().ok())
		.ok_or("Missing Authorization header")?;

	let token = auth_header.strip_prefix("Bearer ")
		.ok_or("Invalid Authorization header format")?;

	// Перевірка токена та отримання ID користувача
	let jwt_auth = JwtAuth::new(b"your-secret-key");
	let claims = jwt_auth.verify_token(token)?;

	// Завантаження користувача з БД за claims.user_id
	let user = User::find_by_id(&db, &claims.user_id).await?;

	// Перевірка активності користувача
	if !user.is_active() {
		return Err("User account is inactive".into());
	}

	// Повернення профілю користувача як JSON
	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

### Визначення ендпоінтів

Reinhardt використовує декоратори HTTP-методів для визначення ендпоінтів:

#### Декоратори HTTP-методів

Використовуйте `#[get]`, `#[post]`, `#[put]`, `#[delete]` для визначення маршрутів:

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

**Можливості:**
- Перевірка шляху під час компіляції
- Лаконічний синтаксис
- Автоматична прив'язка HTTP-методів
- Підтримка впровадження залежностей через `#[inject]`

#### Використання впровадження залежностей

Комбінуйте декоратори HTTP-методів з `#[inject]` для автоматичного впровадження залежностей:

```rust
use reinhardt::{get, Request, Response, StatusCode, ViewResult};
use reinhardt::db::DatabaseConnection;
use std::sync::Arc;

#[get("/users/{id}/", name = "get_user")]
pub async fn get_user(
	req: Request,
	#[inject] db: Arc<DatabaseConnection>,  // Автоматично впроваджується
) -> ViewResult<Response> {
	let id = req.path_params.get("id")
		.ok_or("Missing id")?
		.parse::<i64>()?;

	// Використання впровадженого з'єднання з БД
	let user = db.query("SELECT * FROM users WHERE id = $1")
		.bind(id)
		.fetch_one()
		.await?;

	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

**Можливості впровадження залежностей:**
- Автоматичне впровадження залежностей через атрибут `#[inject]`
- Управління кешем через `#[inject(cache = false)]`
- Система впровадження залежностей, натхненна FastAPI
- Безшовна робота з декораторами HTTP-методів

**Тип значення, що повертається:**

Усі функції представлення використовують `ViewResult<T>` як тип значення, що повертається:

```rust
use reinhardt::ViewResult;  // Попередньо визначений тип результату
```

### З вилученням параметрів

У `views/user.rs` вашого застосунку:

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
	// Вилучення параметра шляху із запиту
	let id = req.path_params.get("id")
		.ok_or("Missing id parameter")?
		.parse::<i64>()
		.map_err(|_| "Invalid id format")?;

	// Вилучення query параметрів (наприклад, ?include_inactive=true)
	let include_inactive = req.query_params.get("include_inactive")
		.and_then(|v| v.parse::<bool>().ok())
		.unwrap_or(false);

	// Отримання користувача з БД з використанням впровадженого з'єднання
	let user = User::find_by_id(&db, id).await?;

	// Перевірка статусу активності за потреби
	if !include_inactive && !user.is_active {
		return Err("User is inactive".into());
	}

	// Повернення як JSON
	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

Зареєструйте маршрут з параметром шляху в `urls.rs`:

```rust
// users/urls.rs
use reinhardt::ServerRouter;

use super::views;

pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::get_user)  // Шлях визначений у #[get("/users/{id}/")]
}
```

### З серіалізаторами та валідацією

У `serializers/user.rs` вашого застосунку:

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

У `views/user.rs` вашого застосунку:

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
	// Парсинг тіла запиту
	let body_bytes = std::mem::take(&mut req.body);
	let create_req: CreateUserRequest = serde_json::from_slice(&body_bytes)?;

	// Валідація запиту
	create_req.validate()?;

	// Створення користувача
	let mut user = User {
		id: 0, // Буде встановлено БД
		username: create_req.username,
		email: create_req.email,
		password_hash: None,
		is_active: true,
		created_at: Utc::now(),
	};

	// Хешування пароля з використанням трейта BaseUser
	user.set_password(&create_req.password)?;

	// Збереження в БД з використанням впровадженого з'єднання
	user.save(&db).await?;

	// Перетворення у відповідь
	let response_data = UserResponse::from(user);
	let json = serde_json::to_string(&response_data)?;

	Ok(Response::new(StatusCode::CREATED)
		.with_body(json))
}
```

## Доступні компоненти

Reinhardt пропонує модульні компоненти для комбінування:

| Компонент            | Назва крейта              | Функції                                     |
|---------------------|---------------------------|---------------------------------------------|
| **Ядро**            |                           |                                             |
| Основні типи        | `reinhardt-core`          | Основні трейти, типи, макроси (Model, endpoint)|
| HTTP та маршрутизація| `reinhardt-http`         | Request/Response, обробка HTTP              |
| URL маршрутизація   | `reinhardt-urls`          | Функціональні та класові маршрути           |
| Сервер              | `reinhardt-server`        | Реалізація HTTP сервера                     |
| Middleware          | `reinhardt-dispatch`      | Ланцюжок middleware, диспетчеризація сигналів|
| Конфігурація        | `reinhardt-conf`          | Управління налаштуваннями, завантаження середовища|
| Команди             | `reinhardt-commands`      | CLI інструменти управління (startproject та ін.)|
| Шорткати            | `reinhardt-shortcuts`     | Загальні утилітарні функції                 |
| **База даних**      |                           |                                             |
| ORM                 | `reinhardt-db`            | Інтеграція reinhardt-query                  |
| **Автентифікація**  |                           |                                             |
| Auth                | `reinhardt-auth`          | JWT, Token, Session, Basic auth, моделі User|
| **REST API**        |                           |                                             |
| Серіалізатори       | `reinhardt-rest`          | Інтеграція serde/validator, ViewSets        |
| **Форми**           |                           |                                             |
| Форми               | `reinhardt-forms`         | Обробка та валідація форм                   |
| **Розширені**       |                           |                                             |
| Адмін-панель        | `reinhardt-admin`         | Інтерфейс адміністрування у стилі Django    |
| Система плагінів    | `reinhardt-dentdelion`    | Статичні та WASM плагіни, CLI управління    |
| Фонові завдання     | `reinhardt-tasks`         | Черги завдань (Redis, RabbitMQ, SQLite)     |
| GraphQL             | `reinhardt-graphql`       | Генерація схем, підписки                    |
| WebSockets          | `reinhardt-websockets`    | Комунікація в реальному часі                |
| i18n                | `reinhardt-i18n`          | Підтримка багатомовності                    |
| **Тестування**      |                           |                                             |
| Утиліти тестування  | `reinhardt-test`          | Хелпери тестування, фікстури, TestContainers|

**Детальні прапорці функцій у кожному крейті див. у [Посібнику з прапорців функцій](../FEATURE_FLAGS.md).**

---

## Документація

- 📚 [Посібник початківця](../GETTING_STARTED.md) - Покрокове керівництво для початківців
- 🎛️ [Посібник з прапорців функцій](../FEATURE_FLAGS.md) - Оптимізація збірки з гранулярним контролем функцій
- 📖 [Довідник API](https://docs.rs/reinhardt) (Скоро)
- 📝 [Підручники](../tutorials/) - Навчання на реальних застосунках

**Для AI асистентів**: Див. [CLAUDE.md](../../CLAUDE.md) для специфічних стандартів кодування, рекомендацій з тестування та угод розробки.

## 💬 Отримання допомоги

Reinhardt — проєкт, керований спільнотою. Ось де можна отримати допомогу:

- 💬 **Discord**: Приєднуйтесь до нашого Discord сервера для спілкування в реальному часі (скоро)
- 💭 **GitHub Discussions**: [Ставте запитання та діліться ідеями](https://github.com/kent8192/reinhardt-web/discussions)
- 🐛 **Issues**: [Повідомляйте про помилки](https://github.com/kent8192/reinhardt-web/issues)
- 📖 **Документація**: [Читайте керівництва](../)

Перед тим як поставити запитання, перевірте:

- ✅ [Посібник початківця](../GETTING_STARTED.md)
- ✅ [Приклади](../../examples/)
- ✅ Існуючі GitHub Issues та Discussions

## 🤝 Внесок у проєкт

Ми любимо внески! Прочитайте [Посібник з внеску](../../CONTRIBUTING.md) для початку.

**Швидкі посилання**:

- [Налаштування розробки](../../CONTRIBUTING.md#development-setup)
- [Керівництво з тестування](../../CONTRIBUTING.md#testing-guidelines)
- [Керівництво з комітів](../../CONTRIBUTING.md#commit-guidelines)

## ⭐ Історія зірок

<a href="https://star-history.com/#kent8192/reinhardt-web&Date">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date&theme=dark" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date" />
   <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date" width="600" />
 </picture>
</a>

## Ліцензія

Цей проєкт ліцензовано за [BSD 3-Clause License](../../LICENSE).

### Атрибуція третіх сторін

Цей проєкт натхненний:

- [Django](https://www.djangoproject.com/) (Ліцензія BSD 3-Clause)
- [Django REST Framework](https://www.django-rest-framework.org/) (Ліцензія BSD 3-Clause)
- [FastAPI](https://fastapi.tiangolo.com/) (Ліцензія MIT)
- [SQLAlchemy](https://www.sqlalchemy.org/) (Ліцензія MIT)

Повну атрибуцію див. у [THIRD-PARTY-NOTICES](../../THIRD-PARTY-NOTICES).

**Примітка:** Цей проєкт не пов'язаний і не схвалений Django Software Foundation, Encode OSS Ltd., Sebastián Ramírez (автор FastAPI) або Michael Bayer (автор SQLAlchemy).
