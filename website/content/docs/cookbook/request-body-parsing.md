+++
title = "Request Body Parsing"
weight = 50
+++

# Request Body Parsing

Guide to extracting data from request bodies and converting to type-safe structs.

## Table of Contents

- [JSON Extraction](#json-extraction)
- [Form Data Extraction](#form-data-extraction)
- [Multipart Extraction](#multipart-extraction)
- [Raw Body Extraction](#raw-body-extraction)
- [Error Handling](#error-handling)

---

## JSON Extraction

### `Json<T>`

Extracts and deserializes JSON from request body.

```rust
use reinhardt::Json;
use serde::Deserialize;

#[derive(Deserialize)]
struct CreateUser {
    username: String,
    email: String,
}

async fn create_user(Json(user): Json<CreateUser>) -> reinhardt::Response {
    reinhardt::Response::ok()
        .with_json(&serde_json::json!({
            "username": user.username,
            "email": user.email
        }))
        .unwrap()
}
```

### Nested JSON

```rust
use reinhardt::Json;
use serde::Deserialize;

#[derive(Deserialize)]
struct Address {
    street: String,
    city: String,
}

#[derive(Deserialize)]
struct CreateUser {
    username: String,
    address: Address,
}

async fn create_user(Json(user): Json<CreateUser>) -> reinhardt::Response {
    reinhardt::Response::ok().with_json(&user).unwrap()
}
```

---

## Form Data Extraction

### `Form<T>`

Extracts `application/x-www-form-urlencoded` data.

```rust
use reinhardt::Form;
use serde::Deserialize;

#[derive(Deserialize)]
struct ContactForm {
    name: String,
    email: String,
    message: String,
}

async fn contact(Form(form): Form<ContactForm>) -> reinhardt::Response {
    reinhardt::Response::ok()
        .with_json(&serde_json::json!({
            "message": "Thank you for contacting us!"
        }))
        .unwrap()
}
```

### Form Validation

```rust
use reinhardt::{Form, Validation};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Deserialize, Validate)]
struct ContactForm {
    #[validate(length(min = 1, max = 100))]
    name: String,

    #[validate(email)]
    email: String,

    #[validate(length(min = 10))]
    message: String,
}

async fn contact(Form(form): Form<ContactForm>) -> reinhardt::Response {
    if let Err(errors) = form.validate() {
        return reinhardt::Response::bad_request()
            .with_json(&serde_json::json!({ "errors": errors }))
            .unwrap();
    }

    reinhardt::Response::ok()
        .with_json(&serde_json::json!({ "status": "success" }))
        .unwrap()
}
```

---

## Multipart Extraction

### `Multipart`

Extracts `multipart/form-data` (used for file uploads).

```rust
use reinhardt::Body;
use multer::Field;

async fn upload_file(mut multipart: Multipart) -> reinhardt::Response {
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("unknown").to_string();

        if name == "file" {
            // Get file data
            let data = field.bytes().await.unwrap();

            return reinhardt::Response::ok()
                .with_json(&serde_json::json!({
                    "size": data.len(),
                    "name": name
                }))
                .unwrap();
        }
    }

    reinhardt::Response::bad_request()
}
```

### Multiple Fields and Files

```rust
use reinhardt::Multipart;

async fn upload_with_metadata(mut multipart: Multipart) -> reinhardt::Response {
    let mut title = None;
    let mut file_data = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("unknown").to_string();

        match name.as_str() {
            "title" => {
                let value = field.text().await.unwrap();
                title = Some(value);
            }
            "file" => {
                let data = field.bytes().await.unwrap();
                file_data = Some(data);
            }
            _ => {}
        }
    }

    reinhardt::Response::ok()
        .with_json(&serde_json::json!({
            "title": title,
            "file_size": file_data.map(|d| d.len())
        }))
        .unwrap()
}
```

---

## Raw Body Extraction

### `Body`

Extracts raw request body as `Bytes`.

```rust
use reinhardt::Body;
use bytes::Bytes;

async fn echo_raw(Body(body): Body) -> reinhardt::Response {
    reinhardt::Response::ok().with_body(body)
}
```

### Body as String

Extract the raw body and convert to `String`.

```rust
use reinhardt::Body;

async fn process_text(Body(body): Body) -> reinhardt::Response {
    let text = String::from_utf8_lossy(&body).to_uppercase();
    reinhardt::Response::ok().with_body(text)
}
```

---

## Error Handling

### Deserialization Errors

JSON parse errors are automatically handled with detailed error messages.

```rust
use reinhardt::Json;
use serde::Deserialize;

#[derive(Deserialize)]
struct User {
    #[serde(rename = "username")]
    name: String,
}

// For invalid JSON:
// {
//   "error": "Failed to deserialize JSON as User",
//   "details": "missing field `username` at line 1 column 10"
// }
async fn create_user(Json(user): Json<User>) -> reinhardt::Response {
    reinhardt::Response::ok().with_json(&user).unwrap()
}
```

### Custom Error Handling

```rust
use reinhardt::Json;
use reinhardt::{Request, Response};
use reinhardt::di::ParamContext;

async fn create_user_manual(req: Request) -> reinhardt::Result<Response> {
    use reinhardt::extract::FromRequest;

    let ctx = ParamContext::new();

    match Json::<User>::from_request(&req, &ctx).await {
        Ok(Json(user)) => {
            Ok(Response::ok().with_json(&user)?)
        }
        Err(e) => {
            Ok(Response::bad_request()
                .with_json(&serde_json::json!({
                    "error": "Invalid JSON",
                    "message": e.to_string()
                }))?
            )
        }
    }
}
```

---

## Multiple Parameters

### Combining Multiple Extractors

```rust
use reinhardt::{Json, Path, Query};

#[derive(serde::Deserialize)]
struct UpdateUser {
    display_name: String,
}

#[derive(serde::Deserialize)]
struct Filter {
    verbose: bool,
}

async fn update_user(
    Path(id): Path<u32>,
    Query(filter): Query<Filter>,
    Json(update): Json<UpdateUser>,
) -> reinhardt::Response {
    reinhardt::Response::ok()
        .with_json(&serde_json::json!({
            "id": id,
            "update": update,
            "verbose": filter.verbose
        }))
        .unwrap()
}
```

---

## See Also

- [Request API](https://docs.rs/reinhardt-http/latest/reinhardt_http/struct.Request.html)
- [Response Serialization](../response-serialization/)
- [Path Parameters](https://docs.rs/reinhardt-urls/latest/reinhardt_urls/routers/struct.ServerRouter.html)
