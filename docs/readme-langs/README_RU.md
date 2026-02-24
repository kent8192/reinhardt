<div align="center">
  <img src="../../branding/logo.png" alt="Reinhardt Logo" width="200"/>

  <h1>Reinhardt</h1>

  <h3>🦀 Полилитический фреймворк с батарейками</h3>

  <p><strong>Компонуемый полнофункциональный API-фреймворк для Rust</strong></p>
  <p>Стройте со <em>всей</em> мощью философии Django "батарейки в комплекте",<br/>
  или компонуйте <em>только</em> то, что вам нужно — ваш выбор, ваш путь.</p>

🌐 [English](../../README.md) | [日本語](README_JA.md) | [简体中文](README_ZH_CN.md) | [繁體中文](README_ZH_TW.md) | **Русский** | [Українська](README_UK.md) | [فارسی](README_FA.md) | [العربية](README_AR.md)

[![Crates.io](https://img.shields.io/crates/v/reinhardt-web.svg)](https://crates.io/crates/reinhardt-web)
[![Documentation](https://docs.rs/reinhardt-web/badge.svg)](https://docs.rs/reinhardt-web)
[![License](https://img.shields.io/badge/license-BSD--3--Clause-blue.svg)](../../LICENSE)
[![codecov](https://codecov.io/gh/kent8192/reinhardt-web/graph/badge.svg)](https://codecov.io/gh/kent8192/reinhardt-web)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/kent8192/reinhardt-web)

</div>

---

## 📍 Быстрая навигация

Возможно, вы ищете:

- 🚀 [Быстрый старт](#быстрый-старт) - Запуск за 5 минут
- 📦 [Варианты установки](#установка) - Выберите свой вариант: Micro, Standard или Full
- 📚 [Руководство по началу работы](../GETTING_STARTED.md) - Пошаговое руководство
- 🎛️ [Флаги функций](../FEATURE_FLAGS.md) - Тонкая настройка сборки
- 📖 [API документация](https://docs.rs/reinhardt-web) - Полный справочник API
- 💬 [Сообщество и поддержка](#получение-помощи) - Получите помощь от сообщества

## Почему Reinhardt?

**Polylithic = Poly (много) + Lithic (строительные блоки)**
В отличие от монолитных фреймворков, которые заставляют вас использовать всё, Reinhardt позволяет компоновать идеальный стек из независимых, хорошо протестированных компонентов.

Reinhardt объединяет лучшее из трёх миров:

| Вдохновение        | Что мы позаимствовали                                  | Что мы улучшили                                     |
|--------------------|--------------------------------------------------------|------------------------------------------------------|
| 🐍 **Django**      | Философия "батарейки в комплекте", дизайн ORM, админка | Флаги функций для компонуемых сборок, типобезопасность Rust |
| 🎯 **Django REST** | Сериализаторы, ViewSets, разрешения                    | Проверка во время компиляции, абстракции с нулевой стоимостью |
| ⚡ **FastAPI**      | DI система, автоматический OpenAPI                      | Нативная производительность Rust, без накладных расходов во время выполнения |
| 🗄️ **SQLAlchemy** | Паттерны QuerySet, обработка связей                     | Типобезопасный конструктор запросов, проверка во время компиляции |

**Результат**: Фреймворк, знакомый Python-разработчикам, но с производительностью и гарантиями безопасности Rust.

## ✨ Ключевые функции

- **Типобезопасная ORM** с проверкой во время компиляции (reinhardt-query)
- **Мощные сериализаторы** с автоматической валидацией (serde + validator)
- **DI в стиле FastAPI** с типобезопасным внедрением зависимостей и кэшированием
- **ViewSets** для быстрой разработки CRUD API
- **Множественная аутентификация** (JWT, Token, Session, Basic) с трейтами BaseUser/FullUser
- **Админ-панель** с автоматически генерируемым интерфейсом управления моделями
- **Команды управления** для миграций, статических файлов и прочего
- **GraphQL и WebSocket** поддержка для приложений реального времени
- **Пагинация, фильтрация, ограничение скорости** встроены
- **Сигналы** для событийно-ориентированной архитектуры

Полный список см. в [Доступные компоненты](#доступные-компоненты), примеры в [Руководстве по началу работы](../GETTING_STARTED.md).

## Установка

Reinhardt — модульный фреймворк. Выберите точку старта:

**Примечание о названии крейтов:**
Основной крейт Reinhardt опубликован на crates.io как `reinhardt-web`, но вы импортируете его как `reinhardt` в коде, используя атрибут `package`.

### По умолчанию: Полнофункциональный (Батарейки в комплекте) ⚠️ Новый default

Все функции без настройки:

```toml
[dependencies]
# Импортируется как 'reinhardt', опубликован как 'reinhardt-web'
# По умолчанию включены ВСЕ функции (полный комплект)
reinhardt = { version = "0.1.0-alpha.1", package = "reinhardt-web" }
```

**Включает:** Database, Auth, REST API, Admin, GraphQL, WebSockets, Cache, i18n, Mail, Sessions, Static Files, Storage

**Бинарник**: ~50+ МБ | **Компиляция**: Медленнее, но всё работает из коробки

Затем используйте в коде:
```rust
use reinhardt::prelude::*;
use reinhardt::{Request, Response, StatusCode};
```

### Вариант 1: Стандартная установка (Сбалансированный)

Для большинства проектов, которым не нужны все функции:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", package = "reinhardt-web", default-features = false, features = ["standard"] }
```

**Включает:** Core, Database (PostgreSQL), REST API, Auth, Middleware, Pages (WASM фронтенд с SSR)

**Бинарник**: ~20-30 МБ | **Компиляция**: Средняя

### Вариант 2: Микросервисы (Минимальная установка)

Лёгкий и быстрый, идеален для простых API:

```toml
[dependencies]
reinhardt = { version = "0.1.0-alpha.1", package = "reinhardt-web", default-features = false, features = ["minimal"] }
```

**Включает:** HTTP, маршрутизация, DI, извлечение параметров, сервер

**Бинарник**: ~5-10 МБ | **Компиляция**: Очень быстрая

### Вариант 3: Создайте свой стек

Устанавливайте только нужные компоненты:

```toml
[dependencies]
# Основные компоненты
reinhardt-http = "0.1.0-alpha.1"
reinhardt-urls = "0.1.0-alpha.1"

# Опционально: База данных
reinhardt-db = "0.1.0-alpha.1"

# Опционально: Аутентификация
reinhardt-auth = "0.1.0-alpha.1"

# Опционально: REST API функции
reinhardt-rest = "0.1.0-alpha.1"

# Опционально: Админ-панель
reinhardt-admin = "0.1.0-alpha.1"

# Опционально: Расширенные функции
reinhardt-graphql = "0.1.0-alpha.1"
reinhardt-websockets = "0.1.0-alpha.1"
```

**📖 Полный список доступных крейтов и флагов функций см. в [Руководстве по флагам функций](../FEATURE_FLAGS.md).**

## Быстрый старт

### 1. Установите Reinhardt Admin Tool

```bash
cargo install reinhardt-admin-cli
```

### 2. Создайте новый проект

```bash
# Создание RESTful API проекта (по умолчанию)
reinhardt-admin startproject my-api
cd my-api
```

Это создаст полную структуру проекта:

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

**Альтернатива: Создание reinhardt-pages проекта (WASM + SSR)**

Для современного WASM-фронтенда с SSR:

```bash
# Создание pages проекта
reinhardt-admin startproject my-app --with-pages
cd my-app

# Установка WASM инструментов сборки (только первый раз)
cargo make install-wasm-tools

# Сборка WASM и запуск сервера разработки
cargo make dev
# Откройте http://127.0.0.1:8000/
```

### 3. Запустите сервер разработки

```bash
# Используя команду manage
cargo run --bin manage runserver

# Сервер запустится на http://127.0.0.1:8000
```

**Поддержка автоперезагрузки:**

Для автоматической перезагрузки при изменении кода (требуется bacon):

```bash
# Установка bacon
cargo install --locked bacon

# Запуск с автоперезагрузкой
bacon runserver

# Или используйте cargo make
cargo make watch

# Для тестов
bacon test
```

### 4. Создайте первое приложение

```bash
# Создание RESTful API приложения (по умолчанию)
cargo run --bin manage startapp users

# Или явно укажите тип
cargo run --bin manage startapp users --restful

# Создание Pages приложения (WASM + SSR)
cargo run --bin manage startapp dashboard --with-pages
```

Это создаст структуру приложения:

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

### 5. Зарегистрируйте маршруты

Отредактируйте `urls.rs` вашего приложения:

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

Включите в `src/config/urls.rs`:

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

Атрибутный макрос `#[routes]` автоматически регистрирует эту функцию во фреймворке для обнаружения через крейт `inventory`.

**Примечание:** `reinhardt::prelude` включает часто используемые типы. Основные экспорты:

**Всегда доступны:**
- Базовая маршрутизация и представления: `Router`, `DefaultRouter`, `ServerRouter`, `View`, `ListView`, `DetailView`
- ViewSets: `ViewSet`, `ModelViewSet`, `ReadOnlyModelViewSet`
- HTTP: `StatusCode`

**Зависят от функций:**
- **Функция `core`**: `Request`, `Response`, `Handler`, `Middleware`, Сигналы (`post_save`, `pre_save` и др.)
- **Функция `database`**: `Model`, `DatabaseConnection`, `F`, `Q`, `Transaction`, `atomic`, Функции БД (`Concat`, `Upper`, `Lower`, `Now`, `CurrentDate`), Оконные функции (`Window`, `RowNumber`, `Rank`, `DenseRank`), Ограничения (`UniqueConstraint`, `CheckConstraint`, `ForeignKeyConstraint`)
- **Функция `auth`**: `User`, `UserManager`, `GroupManager`, `Permission`, `ObjectPermission`
- **Функции `minimal`, `standard` или `di`**: `Body`, `Cookie`, `Header`, `Json`, `Path`, `Query`
- **Функция `rest`**: Сериализаторы, Парсеры, Пагинация, Троттлинг, Версионирование
- **Функция `admin`**: Компоненты админ-панели
- **Функция `cache`**: `Cache`, `InMemoryCache`
- **Функция `sessions`**: `Session`, `AuthenticationMiddleware`

Полный список см. в [Руководстве по флагам функций](../FEATURE_FLAGS.md).

Полное пошаговое руководство см. в [Руководстве по началу работы](../GETTING_STARTED.md).

## 🎓 Учитесь на примерах

### С базой данных

Настройте базу данных в `settings/base.toml`:

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

Настройки автоматически загружаются в `src/config/settings.rs`:

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

**Источники переменных окружения:**

Reinhardt предоставляет два типа источников переменных окружения с разными приоритетами:

- **`EnvSource`** (приоритет: 100) - Высокоприоритетные переменные окружения, которые переопределяют TOML файлы
  ```rust
  .add_source(EnvSource::new().with_prefix("REINHARDT_"))
  ```

- **`LowPriorityEnvSource`** (приоритет: 40) - Низкоприоритетные переменные окружения, которые используются как запасной вариант
  ```rust
  .add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
  ```

**Порядок приоритетов**:
- С `EnvSource`: Переменные окружения > `{profile}.toml` > `base.toml` > Значения по умолчанию
- С `LowPriorityEnvSource` (показано выше): `{profile}.toml` > `base.toml` > Переменные окружения > Значения по умолчанию

Выбирайте `EnvSource`, когда переменные окружения всегда должны иметь приоритет (например, production).
Выбирайте `LowPriorityEnvSource`, когда TOML файлы должны быть основным источником конфигурации (например, разработка).

См. [Документацию по настройкам](../SETTINGS_DOCUMENT.md) для деталей.

**Использование встроенного DefaultUser:**

Reinhardt предоставляет готовую реализацию `DefaultUser` (требуется функция `argon2-hasher`):

```rust
// users/models.rs
use reinhardt::prelude::*;
use reinhardt::DefaultUser;

// Реэкспортируйте DefaultUser как User для вашего приложения
pub type User = DefaultUser;

// DefaultUser включает:
// - id: Uuid (первичный ключ)
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

// DefaultUser реализует:
// - Трейт BaseUser (методы аутентификации)
// - Трейт FullUser (полная информация о пользователе)
// - Трейт PermissionsMixin (управление разрешениями)
// - Трейт Model (операции с БД)
```

**Определение пользовательских моделей:**

Если нужны дополнительные поля, определите свою модель:

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

	// Добавьте пользовательские поля
	#[field(max_length = 50, null = true)]
	pub phone_number: Option<String>,
}
```

**Атрибутный макрос Model:**

Атрибут `#[model(...)]` автоматически генерирует:
- Реализацию трейта `Model` (включает функциональность `#[derive(Model)]`)
- Типобезопасные аксессоры полей: `User::field_email()`, `User::field_username()` и др.
- Регистрацию в глобальном реестре моделей
- Поддержку составных первичных ключей

**Примечание:** При использовании `#[model(...)]` НЕ нужно добавлять `#[derive(Model)]` отдельно,
так как он автоматически применяется атрибутом `#[model(...)]`.

**Атрибуты полей:**
- `#[field(primary_key = true)]` - Отметить как первичный ключ
- `#[field(max_length = 255)]` - Установить максимальную длину для строковых полей
- `#[field(default = value)]` - Установить значение по умолчанию
- `#[field(auto_now_add = true)]` - Автозаполнение timestamp при создании
- `#[field(auto_now = true)]` - Автообновление timestamp при сохранении
- `#[field(null = true)]` - Разрешить NULL значения
- `#[field(unique = true)]` - Применить ограничение уникальности

Полный список атрибутов полей см. в [Руководстве по атрибутам полей](../field_attributes.md).

Сгенерированные аксессоры полей позволяют типобезопасно ссылаться на поля в запросах:

```rust
// Сгенерировано #[model(...)] для DefaultUser
impl DefaultUser {
	pub const fn field_id() -> FieldRef<DefaultUser, Uuid> { FieldRef::new("id") }
	pub const fn field_username() -> FieldRef<DefaultUser, String> { FieldRef::new("username") }
	pub const fn field_email() -> FieldRef<DefaultUser, String> { FieldRef::new("email") }
	pub const fn field_is_active() -> FieldRef<DefaultUser, bool> { FieldRef::new("is_active") }
	pub const fn field_is_staff() -> FieldRef<DefaultUser, bool> { FieldRef::new("is_staff") }
	pub const fn field_date_joined() -> FieldRef<DefaultUser, DateTime<Utc>> { FieldRef::new("date_joined") }
	// ... другие поля
}
```

**Примеры расширенных запросов:**

```rust
use reinhardt::prelude::*;
use reinhardt::DefaultUser;

// Django-стиль F/Q объектных запросов с типобезопасными ссылками на поля
async fn complex_user_query() -> Result<Vec<DefaultUser>, Box<dyn std::error::Error>> {
	// Q объекты с типобезопасными ссылками на поля (используя сгенерированные аксессоры)
	let active_query = Q::new()
		.field("is_active").eq(true)
		.and(Q::new().field("date_joined").gte(Now::new()));

	// Функции БД с типобезопасными ссылками на поля
	let email_lower = Lower::new(DefaultUser::field_email().into());
	let username_upper = Upper::new(DefaultUser::field_username().into());

	// Агрегации используя аксессоры полей
	let user_count = Aggregate::count(DefaultUser::field_id().into());
	let latest_joined = Aggregate::max(DefaultUser::field_date_joined().into());

	// Оконные функции для ранжирования
	let rank_by_join_date = Window::new()
		.partition_by(vec![DefaultUser::field_is_active().into()])
		.order_by(vec![(DefaultUser::field_date_joined().into(), "DESC")])
		.function(RowNumber::new());

	todo!("Execute query with these components")
}

// Поддержка транзакций
async fn create_user_with_transaction(
	conn: &DatabaseConnection,
	user_data: CreateUserRequest
) -> Result<User, Box<dyn std::error::Error>> {
	// Транзакция с автоматическим откатом при ошибке
	transaction(conn, |_tx| async move {
		let user = User::create(user_data).await?;
		log_user_creation(&user).await?;
		Ok(user)
	}).await
}
```

**Примечание**: Reinhardt использует reinhardt-query для SQL операций. Макрос `#[derive(Model)]` автоматически генерирует реализации трейта Model, типобезопасные аксессоры полей и регистрацию в глобальном реестре моделей.

Зарегистрируйте в `src/config/apps.rs`:

```rust
// src/config/apps.rs
use reinhardt::installed_apps;

// Макрос installed_apps! генерирует:
// - Enum InstalledApp с вариантами для каждого приложения
// - Реализацию конверсионных трейтов (From, Into, Display)
// - Реестр для конфигурации и обнаружения приложений
//
// Примечание: В отличие от INSTALLED_APPS Django, этот макрос только для пользовательских приложений.
// Встроенные функции фреймворка (auth, sessions, admin и др.) включаются через
// флаги функций Cargo, а не через installed_apps!.
//
// Пример:
// [dependencies]
// reinhardt = { version = "0.1", features = ["auth", "sessions", "admin"] }
//
// Это включает:
// - Автоматическое обнаружение приложений для миграций, админ-панели и др.
// - Типобезопасные ссылки на приложения в коде
// - Централизованную конфигурацию приложений
installed_apps! {
	users: "users",
}

pub fn get_installed_apps() -> Vec<String> {
	InstalledApp::all_apps()
}
```

### С аутентификацией

Reinhardt предоставляет Django-стиль модели пользователей с трейтами `BaseUser` и `FullUser`, а также комплексное управление пользователями через `UserManager`.

**Примечание:** Reinhardt включает встроенную реализацию `DefaultUser`. Вы можете использовать её напрямую или определить свою модель пользователя, как показано ниже.

**Пример управления пользователями:**

```rust
use reinhardt::prelude::*;

// Создание и управление пользователями с UserManager
async fn manage_users() -> Result<(), Box<dyn std::error::Error>> {
	let hasher = Argon2Hasher::new();
	let user_manager = UserManager::new(hasher);

	// Создание нового пользователя
	let user = user_manager.create_user(CreateUserData {
		username: "alice".to_string(),
		email: "alice@example.com".to_string(),
		password: "secure_password".to_string(),
		first_name: Some("Alice".to_string()),
		last_name: Some("Smith".to_string()),
	}).await?;

	// Обновление информации о пользователе
	user_manager.update_user(user.id, UpdateUserData {
		email: Some("alice.smith@example.com".to_string()),
		is_active: Some(true),
		..Default::default()
	}).await?;

	// Управление группами и разрешениями
	let group_manager = GroupManager::new();
	let editors = group_manager.create_group(CreateGroupData {
		name: "editors".to_string(),
	}).await?;

	// Назначение разрешений на уровне объектов
	let permission = ObjectPermission::new("edit", user.id, article.id);
	let perm_checker = ObjectPermissionChecker::new();
	if perm_checker.has_permission(&user, "edit", &article).await? {
		// Пользователь может редактировать статью
	}

	Ok(())
}
```

Используйте встроенный `DefaultUser` в `users/models.rs`:

```rust
// users/models.rs
use reinhardt::DefaultUser;

// Реэкспортируйте DefaultUser как ваш тип User
pub type User = DefaultUser;

// DefaultUser уже реализует:
// - Трейт BaseUser (методы аутентификации)
// - Трейт FullUser (username, email, first_name, last_name и др.)
// - Трейт PermissionsMixin (управление разрешениями)
// - Трейт Model (операции с БД)
```

**Для пользовательских моделей:**

Если нужны дополнительные поля помимо DefaultUser, определите свою:

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

	// Пользовательские поля
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

Используйте JWT аутентификацию в `views/profile.rs` вашего приложения:

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
	// Извлечение JWT токена из заголовка Authorization
	let auth_header = req.headers.get("authorization")
		.and_then(|h| h.to_str().ok())
		.ok_or("Missing Authorization header")?;

	let token = auth_header.strip_prefix("Bearer ")
		.ok_or("Invalid Authorization header format")?;

	// Проверка токена и получение ID пользователя
	let jwt_auth = JwtAuth::new(b"your-secret-key");
	let claims = jwt_auth.verify_token(token)?;

	// Загрузка пользователя из БД по claims.user_id
	let user = User::find_by_id(&db, &claims.user_id).await?;

	// Проверка активности пользователя
	if !user.is_active() {
		return Err("User account is inactive".into());
	}

	// Возврат профиля пользователя как JSON
	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

### Определение эндпоинтов

Reinhardt использует декораторы HTTP-методов для определения эндпоинтов:

#### Декораторы HTTP-методов

Используйте `#[get]`, `#[post]`, `#[put]`, `#[delete]` для определения маршрутов:

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

**Возможности:**
- Проверка пути во время компиляции
- Лаконичный синтаксис
- Автоматическая привязка HTTP-методов
- Поддержка внедрения зависимостей через `#[inject]`

#### Использование внедрения зависимостей

Комбинируйте декораторы HTTP-методов с `#[inject]` для автоматического внедрения зависимостей:

```rust
use reinhardt::{get, Request, Response, StatusCode, ViewResult};
use reinhardt::db::DatabaseConnection;
use std::sync::Arc;

#[get("/users/{id}/", name = "get_user")]
pub async fn get_user(
	req: Request,
	#[inject] db: Arc<DatabaseConnection>,  // Автоматически внедряется
) -> ViewResult<Response> {
	let id = req.path_params.get("id")
		.ok_or("Missing id")?
		.parse::<i64>()?;

	// Использование внедрённого соединения с БД
	let user = db.query("SELECT * FROM users WHERE id = $1")
		.bind(id)
		.fetch_one()
		.await?;

	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

**Возможности внедрения зависимостей:**
- Автоматическое внедрение зависимостей через атрибут `#[inject]`
- Управление кэшем через `#[inject(cache = false)]`
- Система внедрения зависимостей, вдохновлённая FastAPI
- Бесшовная работа с декораторами HTTP-методов

**Тип возвращаемого значения:**

Все функции представления используют `ViewResult<T>` как тип возвращаемого значения:

```rust
use reinhardt::ViewResult;  // Предопределённый тип результата
```

### С извлечением параметров

В `views/user.rs` вашего приложения:

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
	// Извлечение параметра пути из запроса
	let id = req.path_params.get("id")
		.ok_or("Missing id parameter")?
		.parse::<i64>()
		.map_err(|_| "Invalid id format")?;

	// Извлечение query параметров (например, ?include_inactive=true)
	let include_inactive = req.query_params.get("include_inactive")
		.and_then(|v| v.parse::<bool>().ok())
		.unwrap_or(false);

	// Получение пользователя из БД с использованием внедрённого соединения
	let user = User::find_by_id(&db, id).await?;

	// Проверка статуса активности при необходимости
	if !include_inactive && !user.is_active {
		return Err("User is inactive".into());
	}

	// Возврат как JSON
	let json = serde_json::to_string(&user)?;
	Ok(Response::new(StatusCode::OK)
		.with_body(json))
}
```

Зарегистрируйте маршрут с параметром пути в `urls.rs`:

```rust
// users/urls.rs
use reinhardt::ServerRouter;

use super::views;

pub fn url_patterns() -> ServerRouter {
	ServerRouter::new()
		.endpoint(views::get_user)  // Путь определён в #[get("/users/{id}/")]
}
```

### С сериализаторами и валидацией

В `serializers/user.rs` вашего приложения:

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

В `views/user.rs` вашего приложения:

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
	// Парсинг тела запроса
	let body_bytes = std::mem::take(&mut req.body);
	let create_req: CreateUserRequest = serde_json::from_slice(&body_bytes)?;

	// Валидация запроса
	create_req.validate()?;

	// Создание пользователя
	let mut user = User {
		id: 0, // Будет установлен БД
		username: create_req.username,
		email: create_req.email,
		password_hash: None,
		is_active: true,
		created_at: Utc::now(),
	};

	// Хэширование пароля с использованием трейта BaseUser
	user.set_password(&create_req.password)?;

	// Сохранение в БД с использованием внедрённого соединения
	user.save(&db).await?;

	// Преобразование в ответ
	let response_data = UserResponse::from(user);
	let json = serde_json::to_string(&response_data)?;

	Ok(Response::new(StatusCode::CREATED)
		.with_body(json))
}
```

## Доступные компоненты

Reinhardt предлагает модульные компоненты для комбинирования:

| Компонент            | Название крейта            | Функции                                     |
|---------------------|---------------------------|---------------------------------------------|
| **Ядро**            |                           |                                             |
| Основные типы       | `reinhardt-core`          | Основные трейты, типы, макросы (Model, endpoint)|
| HTTP и маршрутизация| `reinhardt-http`          | Request/Response, обработка HTTP            |
| URL маршрутизация   | `reinhardt-urls`          | Функциональные и классовые маршруты         |
| Сервер              | `reinhardt-server`        | Реализация HTTP сервера                     |
| Middleware          | `reinhardt-dispatch`      | Цепочка middleware, диспетчеризация сигналов|
| Конфигурация        | `reinhardt-conf`          | Управление настройками, загрузка окружения  |
| Команды             | `reinhardt-commands`      | CLI инструменты управления (startproject и др.)|
| Шорткаты            | `reinhardt-shortcuts`     | Общие утилитарные функции                   |
| **База данных**     |                           |                                             |
| ORM                 | `reinhardt-db`            | Интеграция reinhardt-query                  |
| **Аутентификация**  |                           |                                             |
| Auth                | `reinhardt-auth`          | JWT, Token, Session, Basic auth, модели User|
| **REST API**        |                           |                                             |
| Сериализаторы       | `reinhardt-rest`          | Интеграция serde/validator, ViewSets        |
| **Формы**           |                           |                                             |
| Формы               | `reinhardt-forms`         | Обработка и валидация форм                  |
| **Расширенные**     |                           |                                             |
| Админ-панель        | `reinhardt-admin`         | Интерфейс администрирования в стиле Django  |
| Система плагинов    | `reinhardt-dentdelion`    | Статические и WASM плагины, CLI управление  |
| Фоновые задачи      | `reinhardt-tasks`         | Очереди задач (Redis, RabbitMQ, SQLite)     |
| GraphQL             | `reinhardt-graphql`       | Генерация схем, подписки                    |
| WebSockets          | `reinhardt-websockets`    | Коммуникация в реальном времени             |
| i18n                | `reinhardt-i18n`          | Поддержка многоязычности                    |
| **Тестирование**    |                           |                                             |
| Утилиты тестирования| `reinhardt-test`          | Хелперы тестирования, фикстуры, TestContainers|

**Детальные флаги функций в каждом крейте см. в [Руководстве по флагам функций](../FEATURE_FLAGS.md).**

---

## Документация

- 📚 [Руководство по началу работы](../GETTING_STARTED.md) - Пошаговое руководство для начинающих
- 🎛️ [Руководство по флагам функций](../FEATURE_FLAGS.md) - Оптимизация сборки с гранулярным контролем функций
- 📖 [Справочник API](https://docs.rs/reinhardt) (Скоро)
- 📝 [Учебники](../tutorials/) - Обучение на реальных приложениях

**Для AI ассистентов**: См. [CLAUDE.md](../../CLAUDE.md) для специфичных стандартов кодирования, рекомендаций по тестированию и соглашений разработки.

## 💬 Получение помощи

Reinhardt — проект, управляемый сообществом. Вот где можно получить помощь:

- 💬 **Discord**: Присоединяйтесь к нашему Discord серверу для общения в реальном времени (скоро)
- 💭 **GitHub Discussions**: [Задавайте вопросы и делитесь идеями](https://github.com/kent8192/reinhardt-web/discussions)
- 🐛 **Issues**: [Сообщайте об ошибках](https://github.com/kent8192/reinhardt-web/issues)
- 📖 **Документация**: [Читайте руководства](../)

Перед тем как задать вопрос, проверьте:

- ✅ [Руководство по началу работы](../GETTING_STARTED.md)
- ✅ [Примеры](../../examples/)
- ✅ Существующие GitHub Issues и Discussions

## 🤝 Вклад в проект

Мы любим вклады! Прочитайте [Руководство по вкладу](../../CONTRIBUTING.md) для начала.

**Быстрые ссылки**:

- [Настройка разработки](../../CONTRIBUTING.md#development-setup)
- [Руководство по тестированию](../../CONTRIBUTING.md#testing-guidelines)
- [Руководство по коммитам](../../CONTRIBUTING.md#commit-guidelines)

## ⭐ История звёзд

<a href="https://star-history.com/#kent8192/reinhardt-web&Date">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date&theme=dark" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date" />
   <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=kent8192/reinhardt-web&type=Date" width="600" />
 </picture>
</a>

## Лицензия

Этот проект лицензирован под [BSD 3-Clause License](../../LICENSE).

### Атрибуция третьих сторон

Этот проект вдохновлён:

- [Django](https://www.djangoproject.com/) (Лицензия BSD 3-Clause)
- [Django REST Framework](https://www.django-rest-framework.org/) (Лицензия BSD 3-Clause)
- [FastAPI](https://fastapi.tiangolo.com/) (Лицензия MIT)
- [SQLAlchemy](https://www.sqlalchemy.org/) (Лицензия MIT)

Полную атрибуцию см. в [THIRD-PARTY-NOTICES](../../THIRD-PARTY-NOTICES).

**Примечание:** Этот проект не связан и не одобрен Django Software Foundation, Encode OSS Ltd., Sebastián Ramírez (автор FastAPI) или Michael Bayer (автор SQLAlchemy).
