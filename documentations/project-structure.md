# Project structure (current)

```
src/
├── main.rs                                       # Tokio runtime entry point
├── lib.rs                                        # Library crate root: declares public modules
├── bootstrap/
│   ├── app.rs                                    # run(): Logging::init(), config install, HttpServer::run
│   ├── console.rs                                # run(): Logging::init(), bootstrap + ConsoleKernel with registered commands
│   └── server.rs                                 # HttpServer — bind + axum::serve from Config
├── framework/                                    # Framework core (future standalone crate)
│   ├── config/                                   # Configuration loader (TOML + env overrides)
│   │   ├── config.rs                             # Config struct + static facade
│   │   └── error.rs                              # Typed loader errors
│   ├── console/                                  # Console command layer
│   │   └── command.rs                            # Command trait + BoxFuture type alias; ConsoleKernel in mod.rs
│   ├── database/                                 # SQLx-based DB layer with named connections
│   │   ├── config.rs                             # DatabaseConfig, ConnectionConfig, DriverKind
│   │   ├── error.rs                              # DatabaseError
│   │   ├── manager.rs                            # Database + Connection enum + static facade
│   │   └── migrate.rs                            # sqlx::Migrator runner
│   ├── http/
│   │   └── middleware/                           # Middleware trait + Axum adapters
│   ├── logging/                                  # Logging::init() — tracing subscriber, declarative sinks
│   │   ├── config.rs                             # LoggingConfig, FileSinkConfig, etc. (parsed from config/logging.toml)
│   │   └── sink/                                 # console, file (rolling + retention), database, queue sinks
│   └── routing/
│       └── route.rs                              # Route builder facade over axum::Router
├── routes/
│   └── api.rs                                    # API route table — register your routes here
└── app/
    ├── console/
    │   └── commands/
    │       └── test_command.rs                   # Example custom command (greet [name])
    └── http/
        ├── controllers/
        │   └── hello_world_controller.rs         # Your controllers go in this directory
        └── middleware/
            └── trim_strings.rs                   # Example TrimStrings middleware

config/                                           # Application configuration (TOML files)
├── app.toml
├── database.toml
└── logging.toml

database/                                         # Schema and data files
└── migrations/                                   # Versioned SQL migrations (sqlx)

.env                                              # Local environment overrides (gitignored)
.env.example                                      # Committed template for required env vars

tests/
├── feature/                                      # Component / integration tests (HTTP, controllers, end-to-end)
│   ├── mod.rs                                    # entry: declares submodule tests
│   ├── config_loads_from_directory.rs
│   ├── hello_world_controller.rs
│   └── trim_strings.rs
└── unit/                                         # Focused unit tests of small components
    ├── mod.rs                                    # entry: declares submodule tests
    ├── config.rs
    ├── hello_payload.rs
    └── trim_strings.rs
```

[← Back to readme](../readme.md)
