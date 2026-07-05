# Add a controller and register its route

This is the most common day-to-day task. It takes three steps.

## 1. Create the controller file

Create a new file `src/app/http/controllers/<name>_controller.rs`. A controller is a unit struct with one or more associated `async fn` handlers. Each handler returns an Axum-compatible response — most commonly `axum::Json<T>` for a JSON payload, where `T` derives `serde::Serialize`.

Example: `src/app/http/controllers/users_controller.rs`

```rust
use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct UserListResponse {
    pub users: Vec<String>,
}

pub struct UsersController;

impl UsersController {
    pub async fn index() -> Json<UserListResponse> {
        Json(UserListResponse {
            users: vec!["alice".into(), "bob".into()],
        })
    }

    pub async fn show() -> Json<&'static str> {
        Json("user details")
    }
}
```

## 2. Expose the controller from the controllers module

Open `src/app/http/controllers/mod.rs` and add two lines: declare the new module and re-export the controller struct.

```rust
pub mod hello_world_controller;
pub mod users_controller;          // <-- added

pub use hello_world_controller::HelloWorldController;
pub use users_controller::UsersController;   // <-- added
```

The `pub use` re-export lets you import the controller with the short path `crate::app::http::controllers::UsersController` instead of the longer `crate::app::http::controllers::users_controller::UsersController`.

## 3. Register the route in `src/routes/api.rs`

Open `src/routes/api.rs` and chain new routes onto the `Route` builder using the appropriate HTTP verb method (`get`, `post`, `put`, `patch`, `delete`).

```rust
use axum::Router;

use crate::app::http::controllers::{HelloWorldController, UsersController};
use crate::framework::routing::Route;

pub fn routes() -> Router {
    Route::new()
        .get("/hello", HelloWorldController::index)
        .get("/users", UsersController::index)        // <-- added
        .get("/users/:id", UsersController::show)     // <-- added
        .into_router()
}
```

That's it. Run `cargo run` and `curl http://127.0.0.1:3000/users` will hit your new handler.

### Available verbs on `Route`

| Method | HTTP verb |
|---|---|
| `.get(path, handler)` | `GET` |
| `.post(path, handler)` | `POST` |
| `.put(path, handler)` | `PUT` |
| `.patch(path, handler)` | `PATCH` |
| `.delete(path, handler)` | `DELETE` |

Each method takes the path string and a handler — typically `SomeController::some_method`. The handler signature must satisfy Axum's `Handler` trait; in practice, any `async fn` returning an `IntoResponse` (e.g. `Json<T>`, `String`, `(StatusCode, Json<T>)`) will work.

See also: [Routing under the hood](routing.md).

[← Back to readme](../readme.md)
