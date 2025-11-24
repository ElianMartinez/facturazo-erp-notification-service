# Arquitectura - PDF Services v2.0

## Visión General

```
                              ┌──────────────────────────────────────┐
                              │           CLIENTS                     │
                              │   (Web, Mobile, External Services)    │
                              └─────────────┬────────────────────────┘
                                            │
                    ┌───────────────────────┼───────────────────────┐
                    │                       │                       │
                    ▼                       ▼                       ▼
            ┌───────────────┐      ┌───────────────┐      ┌───────────────┐
            │   HTTP API    │      │    Kafka      │      │   Webhooks    │
            │   (Sync)      │      │   (Async)     │      │   (Events)    │
            │   :8080       │      │   :9092       │      │               │
            └───────┬───────┘      └───────┬───────┘      └───────┬───────┘
                    │                      │                      │
                    └──────────────────────┼──────────────────────┘
                                           │
                                           ▼
                    ┌──────────────────────────────────────────────┐
                    │              PDF-SERVICES v2.0               │
                    │                                              │
                    │  ┌────────────────────────────────────────┐  │
                    │  │           API LAYER                    │  │
                    │  │  • handlers.rs    • routes.rs          │  │
                    │  │  • middleware/    • state.rs           │  │
                    │  └────────────────────────────────────────┘  │
                    │                      │                       │
                    │  ┌────────────────────────────────────────┐  │
                    │  │        APPLICATION LAYER               │  │
                    │  │  • commands/      • queries/           │  │
                    │  │  • orchestrators/                      │  │
                    │  └────────────────────────────────────────┘  │
                    │                      │                       │
                    │  ┌────────────────────────────────────────┐  │
                    │  │          DOMAIN LAYER                  │  │
                    │  │  • document/      • invoice/           │  │
                    │  │  • notification/  • fiscal/            │  │
                    │  └────────────────────────────────────────┘  │
                    │                      │                       │
                    │  ┌────────────────────────────────────────┐  │
                    │  │       INFRASTRUCTURE LAYER             │  │
                    │  │  • generators/    • notifications/     │  │
                    │  │  • database/      • cache/             │  │
                    │  │  • storage.rs                          │  │
                    │  └────────────────────────────────────────┘  │
                    │                                              │
                    └──────────────────────────────────────────────┘
                                           │
                    ┌──────────────────────┼──────────────────────┐
                    │                      │                      │
                    ▼                      ▼                      ▼
            ┌───────────────┐      ┌───────────────┐      ┌───────────────┐
            │    SQLite     │      │  Cloudflare   │      │  EvolutionAPI │
            │   (Metadata)  │      │      R2       │      │  (WhatsApp)   │
            └───────────────┘      │   (Storage)   │      └───────────────┘
                                   └───────────────┘
                                          │
                                          ▼
                                   ┌───────────────┐
                                   │     SMTP      │
                                   │   (Email)     │
                                   └───────────────┘
```

---

## Estructura de Directorios

```
pdf-services/
├── src/
│   ├── api/                          # REST API Layer
│   │   ├── handlers.rs               # Request handlers
│   │   ├── routes.rs                 # Route definitions
│   │   ├── state.rs                  # AppState compartido
│   │   ├── error.rs                  # Error handling
│   │   ├── template_handler.rs       # Template endpoints
│   │   └── middleware/
│   │       ├── auth.rs               # JWT authentication
│   │       └── compression.rs        # Response compression
│   │
│   ├── application/                  # Application Layer (Use Cases)
│   │   ├── commands/                 # Write operations
│   │   │   └── mod.rs
│   │   ├── queries/                  # Read operations
│   │   │   └── mod.rs
│   │   └── orchestrators/            # Workflow coordination
│   │       └── mod.rs
│   │
│   ├── domain/                       # Domain Layer (Business Logic)
│   │   ├── document/                 # Document aggregate
│   │   │   └── mod.rs
│   │   ├── invoice/                  # Invoice aggregate
│   │   │   └── mod.rs
│   │   ├── fiscal/                   # Fiscal rules (RD)
│   │   │   └── mod.rs
│   │   ├── notification/             # Notification aggregate
│   │   │   └── mod.rs
│   │   └── shared/                   # Shared value objects
│   │       └── mod.rs
│   │
│   ├── infrastructure/               # Infrastructure Layer
│   │   ├── cache/                    # In-memory caching
│   │   │   └── mod.rs
│   │   ├── database/                 # SQLite repositories
│   │   │   ├── document_repository.rs
│   │   │   ├── invoice_repository.rs
│   │   │   ├── ncf_sequence_repository.rs
│   │   │   └── notification_repository.rs
│   │   ├── generators/               # Document generators
│   │   │   ├── typst_generator.rs    # PDF via Typst
│   │   │   ├── invoice_generator.rs  # Fiscal invoices
│   │   │   ├── report_generator.rs   # Reports
│   │   │   ├── excel_generator.rs    # XLSX files
│   │   │   ├── csv_generator.rs      # CSV export
│   │   │   ├── qr_generator.rs       # QR codes
│   │   │   ├── quotation_generator.rs
│   │   │   └── template_manager.rs   # Template versioning
│   │   ├── notifications/            # Notification services
│   │   │   ├── email.rs              # SMTP client
│   │   │   └── evolution_api.rs      # WhatsApp via EvolutionAPI
│   │   ├── storage.rs                # S3/R2 storage
│   │   ├── observability.rs          # Metrics & tracing
│   │   └── persistence.rs            # Persistence abstractions
│   │
│   ├── kafka/                        # Message Bus
│   │   ├── consumer.rs               # Kafka consumer
│   │   ├── producer.rs               # Kafka producer
│   │   └── handlers.rs               # Message handlers
│   │
│   ├── templates/                    # Typst Templates
│   │   ├── template_engine.rs
│   │   ├── template_models.rs
│   │   ├── template_trait.rs
│   │   └── templates/
│   │       ├── fiscal_invoice.rs     # Factura fiscal RD
│   │       ├── simple_invoice.rs     # Factura simple
│   │       ├── receipt.rs            # Recibo
│   │       ├── report.rs             # Reportes
│   │       └── quotation.rs          # Cotizaciones
│   │
│   ├── bin/                          # Executables
│   │   ├── pdf_services.rs           # Main API server
│   │   ├── kafka_worker.rs           # Async worker
│   │   ├── benchmark_report.rs       # Benchmarking tool
│   │   └── quotation_generator.rs    # Quotation tool
│   │
│   ├── config/                       # Configuration
│   │   └── mod.rs
│   │
│   ├── common/                       # Shared utilities
│   │   ├── middleware.rs
│   │   ├── security.rs
│   │   └── utils.rs
│   │
│   ├── models/                       # Data models (legacy)
│   │   ├── document.rs
│   │   ├── invoice.rs
│   │   ├── report.rs
│   │   └── common.rs
│   │
│   ├── generators/                   # Legacy generators
│   │   ├── pdf.rs
│   │   └── excel.rs
│   │
│   ├── storage/                      # Legacy storage
│   │   └── s3.rs
│   │
│   ├── lib.rs
│   └── main.rs
│
├── tests/
│   └── integration_tests.rs
│
├── docs/
│   ├── ARCHITECTURE.md
│   ├── PROJECT_CONTEXT.md
│   ├── IMPLEMENTATION_ROADMAP.md
│   └── CURRENT_SESSION.md
│
├── Cargo.toml
├── Cargo.lock
└── .env.example
```

---

## Flujo de Datos

### Generación Síncrona (HTTP)

```
Client                API                  Orchestrator            Generator           Storage
  │                    │                       │                       │                  │
  │  POST /generate    │                       │                       │                  │
  │───────────────────>│                       │                       │                  │
  │                    │  GenerateDocument     │                       │                  │
  │                    │──────────────────────>│                       │                  │
  │                    │                       │  Generate PDF         │                  │
  │                    │                       │──────────────────────>│                  │
  │                    │                       │                       │                  │
  │                    │                       │      PDF bytes        │                  │
  │                    │                       │<──────────────────────│                  │
  │                    │                       │                       │                  │
  │                    │                       │  Upload to R2                            │
  │                    │                       │─────────────────────────────────────────>│
  │                    │                       │                                          │
  │                    │                       │      Signed URL                          │
  │                    │                       │<─────────────────────────────────────────│
  │                    │                       │                       │                  │
  │                    │   Document URL        │                       │                  │
  │                    │<──────────────────────│                       │                  │
  │                    │                       │                       │                  │
  │   { url: "..." }   │                       │                       │                  │
  │<───────────────────│                       │                       │                  │
```

### Generación Asíncrona (Kafka)

```
Producer              Kafka                Consumer            Orchestrator          Notification
  │                    │                       │                    │                     │
  │  document-request  │                       │                    │                     │
  │───────────────────>│                       │                    │                     │
  │                    │  Consume message      │                    │                     │
  │                    │──────────────────────>│                    │                     │
  │                    │                       │  Process           │                     │
  │                    │                       │───────────────────>│                     │
  │                    │                       │                    │                     │
  │                    │                       │                    │  (generate PDF...)  │
  │                    │                       │                    │                     │
  │                    │                       │  Send notification │                     │
  │                    │                       │                    │────────────────────>│
  │                    │                       │                    │                     │
  │                    │  document-generated   │                    │                     │
  │                    │<──────────────────────│                    │                     │
```

---

## Componentes Principales

### 1. API Layer (`src/api/`)

| Archivo | Responsabilidad |
|---------|-----------------|
| `handlers.rs` | Request/Response handling |
| `routes.rs` | Route configuration |
| `state.rs` | Shared application state |
| `error.rs` | Error responses |
| `middleware/auth.rs` | JWT validation |
| `middleware/compression.rs` | Gzip/Zstd compression |

### 2. Application Layer (`src/application/`)

| Módulo | Responsabilidad |
|--------|-----------------|
| `commands/` | Write operations (generate, send) |
| `queries/` | Read operations (status, list) |
| `orchestrators/` | Complex workflows |

### 3. Domain Layer (`src/domain/`)

| Módulo | Responsabilidad |
|--------|-----------------|
| `document/` | Document entities & rules |
| `invoice/` | Invoice business logic |
| `fiscal/` | RD fiscal compliance |
| `notification/` | Notification entities |
| `shared/` | Common value objects |

### 4. Infrastructure Layer (`src/infrastructure/`)

| Módulo | Responsabilidad |
|--------|-----------------|
| `generators/` | PDF, Excel, CSV, QR generation |
| `notifications/` | Email (SMTP), WhatsApp (EvolutionAPI) |
| `database/` | SQLite repositories |
| `cache/` | In-memory caching |
| `storage.rs` | S3/R2 file storage |

### 5. Kafka Integration (`src/kafka/`)

| Archivo | Responsabilidad |
|---------|-----------------|
| `consumer.rs` | Message consumption |
| `producer.rs` | Event publishing |
| `handlers.rs` | Message routing |

---

## Generadores de Documentos

| Generador | Motor | Formato | Uso |
|-----------|-------|---------|-----|
| `typst_generator.rs` | Typst CLI | PDF | Base para todos los PDFs |
| `invoice_generator.rs` | Typst | PDF | Facturas fiscales RD |
| `report_generator.rs` | Typst | PDF | Reportes tabulares |
| `excel_generator.rs` | rust_xlsxwriter | XLSX | Hojas de cálculo |
| `csv_generator.rs` | std | CSV | Exportación masiva |
| `qr_generator.rs` | qrcode | PNG/Base64 | Códigos QR fiscales |
| `quotation_generator.rs` | Typst | PDF | Cotizaciones |

---

## Templates Typst

```rust
// Trait que implementan todos los templates
pub trait TypstTemplate {
    fn generate(&self, data: &Value) -> Result<String>;
    fn template_id(&self) -> &str;
    fn validate(&self, data: &Value) -> Result<()>;
}
```

| Template | ID | Descripción |
|----------|-----|-------------|
| `fiscal_invoice.rs` | `fiscal_invoice` | Factura fiscal RD (NCF) |
| `simple_invoice.rs` | `simple_invoice` | Factura simple |
| `receipt.rs` | `receipt` | Recibo de pago |
| `report.rs` | `report` | Reportes genéricos |
| `quotation.rs` | `quotation` | Cotizaciones |

---

## Dependencias Clave

```toml
[dependencies]
# Web Framework
actix-web = "4"
actix-rt = "2"

# Async Runtime
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Database
sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio"] }

# Kafka
rdkafka = { version = "0.36", features = ["cmake-build"] }

# Storage
aws-sdk-s3 = "1"

# Document Generation
rust_xlsxwriter = "0.64"
qrcode = "0.14"
image = "0.25"

# Notifications
lettre = "0.11"  # SMTP
reqwest = "0.11" # HTTP client for EvolutionAPI

# Caching
dashmap = "5"

# Observability
tracing = "0.1"
tracing-subscriber = "0.3"
```

---

## Comandos de Ejecución

```bash
# API Server (HTTP :8080)
cargo run --bin pdf-services

# Kafka Worker (Async processing)
cargo run --bin kafka_worker

# Benchmark Tool
cargo run --bin benchmark-report

# Tests
cargo test --lib

# Build Release
cargo build --release
```

---

## Tests

21 tests unitarios cubriendo:

- **Cache**: Operaciones básicas, TTL, manager
- **Generators**: Typst, Invoice, Report, Excel, CSV, QR
- **Templates**: Manager, versioning
- **Notifications**: Phone validation (RD)
- **Kafka**: Message parsing, handlers

```bash
cargo test --lib 2>&1 | grep "test result"
# test result: ok. 21 passed; 0 failed
```

---

*Última actualización: 2024-11-24*
