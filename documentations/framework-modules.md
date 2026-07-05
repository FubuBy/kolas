# Add a new module to the framework core

If you want to extend the framework itself (e.g. add a request-validation layer, an exception handler), create the new sub-module under `src/framework/` and declare it in `src/framework/mod.rs`. HTTP middleware primitives already live under `src/framework/http/middleware/`.

Treat `src/framework/` as future standalone-crate code: avoid depending on anything in `src/app/`, `src/routes/`, or `src/bootstrap/` from inside `framework/`. The dependency direction is one-way — application code depends on the framework, never the reverse.

[← Back to readme](../readme.md)
