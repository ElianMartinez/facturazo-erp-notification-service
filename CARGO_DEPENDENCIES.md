# 📦 Cargo.toml - Dependencias Actualizadas

## Cargo.toml Principal

```toml
[package]
name = "document-notification-service"
version = "1.0.0"
edition = "2021"
rust-version = "1.75"

[dependencies]
# ============================================
# Core & Async Runtime
# ============================================
tokio = { version = "1.35", features = ["full"] }
tokio-util = "0.7"
futures = "0.3"
async-trait = "0.1"

# ============================================
# Web Framework (Actix-web)
# ============================================
actix-web = "4.4"
actix-rt = "2.9"
actix-cors = "0.7"
actix-ws = "0.2"  # Para WebSocket
actix-web-httpauth = "0.8"
actix-files = "0.6"  # Para servir archivos estáticos

# ============================================
# Message Queue (Kafka)
# ============================================
rdkafka = { version = "0.36", features = ["ssl", "sasl", "gssapi", "libz"] }

# ============================================
# Database (SQLite con SQLx)
# ============================================
sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio-rustls", "macros", "uuid", "chrono", "migrate"] }

# ============================================
# In-Memory Cache
# ============================================
dashmap = "5.5"  # Concurrent hashmap para cache
moka = { version = "0.12", features = ["future"] }  # Cache con TTL y eviction
parking_lot = "0.12"  # Mejores locks que std

# ============================================
# Document Generation
# ============================================
# PDF con Typst (necesitarás clonar y compilar localmente)
# typst = "0.11"  # No disponible en crates.io aún
# typst-pdf = "0.11"

# Excel Generation
rust_xlsxwriter = { version = "0.64", features = ["chrono", "zlib"] }

# CSV Generation
csv = "1.3"

# QR Code
qrcode = "0.14"
image = "0.24"

# ============================================
# Notifications
# ============================================
# Email (SMTP)
lettre = { version = "0.11", features = ["tokio1-rustls-tls", "smtp-transport", "builder", "hostname"] }
mail-parser = "0.9"  # Para parsear emails entrantes

# Templates
tera = "1.19"  # Para templates de email/whatsapp
handlebars = "5.1"  # Alternativa a Tera

# ============================================
# Storage (Cloudflare R2 - S3 Compatible)
# ============================================
aws-sdk-s3 = "1.13"  # R2 es compatible con S3
aws-config = "1.1"
aws-credential-types = "1.1"

# ============================================
# HTTP Client (Para EvolutionAPI)
# ============================================
reqwest = { version = "0.11", features = ["json", "gzip", "rustls-tls", "stream", "multipart"] }

# ============================================
# Serialization
# ============================================
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"
toml = "0.8"

# ============================================
# Date/Time
# ============================================
chrono = { version = "0.4", features = ["serde"] }
chrono-tz = "0.8"  # Para timezone de República Dominicana

# ============================================
# UUID & IDs
# ============================================
uuid = { version = "1.6", features = ["v4", "serde", "fast-rng"] }
nanoid = "0.4"  # Para IDs más cortos

# ============================================
# Logging & Observability
# ============================================
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json", "fmt"] }
tracing-actix-web = "0.7"
opentelemetry = { version = "0.21", optional = true }
opentelemetry-otlp = { version = "0.14", optional = true }

# Metrics
prometheus = "0.13"
prometheus-http-metrics = "0.1"

# ============================================
# Configuration
# ============================================
config = "0.14"
dotenv = "0.15"
envy = "0.4"  # Para deserializar env vars a structs

# ============================================
# Error Handling
# ============================================
thiserror = "1.0"
anyhow = "1.0"
color-eyre = "0.6"  # Pretty error reporting

# ============================================
# Validation
# ============================================
validator = { version = "0.18", features = ["derive"] }
garde = { version = "0.18", features = ["derive"] }  # Alternativa moderna

# ============================================
# Security & Auth
# ============================================
jsonwebtoken = "9.2"
argon2 = "0.5"  # Password hashing
ring = "0.17"  # Cryptography
base64 = "0.21"

# Rate Limiting & Circuit Breaker
governor = "0.6"
tower = { version = "0.4", features = ["limit", "timeout", "buffer"] }

# ============================================
# Utilities
# ============================================
# Compression
flate2 = "1.0"
zstd = "0.13"

# Command Line (para herramientas CLI)
clap = { version = "4.4", features = ["derive", "env"] }

# Random
rand = "0.8"

# Regex
regex = "1.10"
once_cell = "1.19"  # Para regex compiladas estáticamente

# ============================================
# Development Dependencies
# ============================================
[dev-dependencies]
mockall = "0.12"
fake = { version = "2.9", features = ["derive"] }
rstest = "0.18"  # Better test fixtures
criterion = { version = "0.5", features = ["html_reports"] }
tempfile = "3.8"
insta = "1.34"  # Snapshot testing
wiremock = "0.6"  # HTTP mocking

# ============================================
# Build Dependencies
# ============================================
[build-dependencies]
vergen = { version = "8.2", features = ["build", "git", "gitcl"] }

# ============================================
# Features
# ============================================
[features]
default = ["sqlite", "evolution-api"]
postgres = ["sqlx/postgres"]
sqlite = ["sqlx/sqlite"]
evolution-api = []  # Feature flag para EvolutionAPI
telemetry = ["opentelemetry", "opentelemetry-otlp"]

# ============================================
# Profile Configurations
# ============================================
[profile.dev]
opt-level = 0
debug = true
incremental = true

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
strip = true
panic = "abort"

[profile.release-with-debug]
inherits = "release"
debug = true
strip = false

# Para benchmarks
[profile.bench]
inherits = "release"

# ============================================
# Workspace Configuration (si usas workspace)
# ============================================
# [workspace]
# members = [
#     "crates/domain",
#     "crates/application",
#     "crates/infrastructure",
#     "crates/api",
# ]

# ============================================
# Binary Targets
# ============================================
[[bin]]
name = "api-server"
path = "src/main.rs"

[[bin]]
name = "kafka-worker"
path = "src/bin/kafka_worker.rs"

[[bin]]
name = "migration"
path = "src/bin/migration.rs"

# ============================================
# Benchmarks
# ============================================
[[bench]]
name = "pdf_generation"
harness = false

[[bench]]
name = "cache_performance"
harness = false
```

## Notas sobre las Dependencias

### Cambios Principales vs Arquitectura Original:

1. **Actix-web** en lugar de Axum
   - Más maduro y estable
   - Mejor integración con el ecosistema actual
   - Excelente performance

2. **SQLite** en lugar de PostgreSQL
   - Embedded, zero-config
   - Perfecto para microservicios
   - SQLx soporta migrations

3. **In-Memory Cache** (dashmap/moka)
   - `dashmap`: HashMap concurrente de alto rendimiento
   - `moka`: Cache con TTL, eviction policies, similar a Caffeine de Java
   - Sin dependencia externa de Redis

4. **EvolutionAPI** para WhatsApp
   - Se integrará via HTTP client (reqwest)
   - Feature flag para habilitar/deshabilitar

5. **SMTP propio** para Email
   - `lettre`: Cliente SMTP robusto y completo
   - Soporte para múltiples proveedores

### Dependencias Opcionales:

- **OpenTelemetry**: Solo si necesitas tracing distribuido
- **PostgreSQL**: Disponible como feature flag si decides migrar

### Para Typst:

Typst aún no está disponible en crates.io, necesitarás:
1. Usar el binario del sistema (`Command::new("typst")`)
2. O compilar como dependencia local:

```toml
[dependencies]
typst = { path = "../typst" }
```

### Optimizaciones de Compilación:

- **Release**: LTO completo, single codegen unit, strip symbols
- **Dev**: Compilación incremental, debug symbols
- **Bench**: Basado en release para benchmarks precisos

## Instalación de Dependencias del Sistema

```bash
# macOS
brew install typst
brew install kafka

# Ubuntu/Debian
apt-get install sqlite3
# Typst se instala manualmente

# Verificar instalaciones
typst --version
sqlite3 --version
```

---
*Este archivo contiene las dependencias actualizadas según los cambios de arquitectura solicitados*