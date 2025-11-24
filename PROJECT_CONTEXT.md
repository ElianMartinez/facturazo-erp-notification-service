# PROJECT CONTEXT: Document & Notification Service

## Estado del Proyecto
- **Versión**: 2.0.0 (v2-microservice branch)
- **Progreso**: 75% completado
- **Tests**: 21 pasando
- **Última actualización**: 2024-11-24

## Objetivo del Proyecto
Microservicio enterprise para generación de documentos (PDF, Excel, CSV) y envío de notificaciones (Email, WhatsApp) de manera síncrona y asíncrona.

---

## Stack Tecnológico

| Componente | Tecnología | Estado |
|------------|------------|--------|
| Lenguaje | Rust | ✅ |
| Runtime Async | Tokio | ✅ |
| API HTTP | Actix-web | ✅ |
| Message Bus | Kafka (rdkafka) | ✅ |
| PDF Engine | Typst | ✅ |
| Excel | rust_xlsxwriter | ✅ |
| Database | SQLite (SQLx) | ✅ |
| Cache | In-memory (dashmap) | ✅ |
| Storage | Cloudflare R2 / S3 | ✅ |
| WhatsApp | EvolutionAPI | ✅ |
| Email | SMTP (lettre) | ✅ |
| Observability | tracing + prometheus | ✅ |

---

## Arquitectura

```
┌─────────────────────────────────────────────────────────────────┐
│                    PDF-SERVICES v2.0                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐         │
│  │  HTTP API   │    │   Kafka     │    │   Workers   │         │
│  │  (Actix)    │    │  Consumer   │    │  (Async)    │         │
│  └──────┬──────┘    └──────┬──────┘    └──────┬──────┘         │
│         │                  │                  │                 │
│         └──────────────────┼──────────────────┘                 │
│                            │                                    │
│                   ┌────────▼────────┐                          │
│                   │   Application   │                          │
│                   │  Orchestrators  │                          │
│                   └────────┬────────┘                          │
│                            │                                    │
│         ┌──────────────────┼──────────────────┐                │
│         │                  │                  │                 │
│  ┌──────▼──────┐   ┌──────▼──────┐   ┌──────▼──────┐          │
│  │  Document   │   │   Domain    │   │Notification │          │
│  │ Generators  │   │   Layer     │   │  Services   │          │
│  └──────┬──────┘   └─────────────┘   └──────┬──────┘          │
│         │                                    │                  │
│  ┌──────▼──────────────────────────────────▼──────┐           │
│  │              Infrastructure Layer               │           │
│  │  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐   │           │
│  │  │ SQLite │ │ Cache  │ │   S3   │ │  SMTP  │   │           │
│  │  └────────┘ └────────┘ └────────┘ └────────┘   │           │
│  │  ┌────────┐ ┌────────┐                         │           │
│  │  │ Typst  │ │Evolution│                        │           │
│  │  └────────┘ │  API   │                         │           │
│  │             └────────┘                         │           │
│  └────────────────────────────────────────────────┘           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Estructura del Proyecto

```
pdf-services/
├── src/
│   ├── api/                     # REST API (Actix-web)
│   │   ├── handlers.rs          # Endpoints handlers
│   │   ├── routes.rs            # Route definitions
│   │   ├── state.rs             # Shared state
│   │   ├── error.rs             # Error handling
│   │   ├── template_handler.rs  # Template endpoints
│   │   └── middleware/          # Auth, compression
│   │
│   ├── application/             # Use cases
│   │   ├── commands/            # Write operations
│   │   ├── queries/             # Read operations
│   │   └── orchestrators/       # Workflow coordination
│   │
│   ├── domain/                  # Business logic
│   │   ├── document/            # Document entities
│   │   ├── invoice/             # Invoice entities
│   │   ├── fiscal/              # Fiscal rules (RD)
│   │   ├── notification/        # Notification entities
│   │   └── shared/              # Common value objects
│   │
│   ├── infrastructure/          # External implementations
│   │   ├── cache/               # In-memory cache
│   │   ├── database/            # SQLite repositories
│   │   ├── generators/          # Document generators
│   │   │   ├── typst_generator.rs
│   │   │   ├── invoice_generator.rs
│   │   │   ├── report_generator.rs
│   │   │   ├── excel_generator.rs
│   │   │   ├── csv_generator.rs
│   │   │   ├── qr_generator.rs
│   │   │   └── template_manager.rs
│   │   ├── notifications/       # Notification services
│   │   │   ├── email.rs         # SMTP
│   │   │   └── evolution_api.rs # WhatsApp
│   │   └── storage.rs           # S3/R2 storage
│   │
│   ├── kafka/                   # Message bus
│   │   ├── consumer.rs          # Kafka consumer
│   │   ├── producer.rs          # Kafka producer
│   │   └── handlers.rs          # Message handlers
│   │
│   ├── templates/               # Typst templates
│   │   └── templates/
│   │       ├── fiscal_invoice.rs
│   │       ├── simple_invoice.rs
│   │       ├── receipt.rs
│   │       ├── report.rs
│   │       └── quotation.rs
│   │
│   ├── bin/                     # Executables
│   │   ├── pdf_services.rs      # Main API server
│   │   ├── kafka_worker.rs      # Async worker
│   │   ├── benchmark_report.rs  # Benchmarking
│   │   └── quotation_generator.rs
│   │
│   └── config/                  # Configuration
│
├── tests/                       # Integration tests
└── docs/                        # Documentation
```

---

## APIs

### HTTP Endpoints (Sync)

| Method | Endpoint | Descripción |
|--------|----------|-------------|
| POST | `/api/v1/generate/sync` | Generación síncrona |
| POST | `/api/v1/generate/async` | Generación asíncrona |
| GET | `/api/v1/documents/{id}` | Estado del documento |
| POST | `/api/v1/templates/generate` | Generar con template |
| GET | `/health` | Health check |

### Kafka Topics (Async)

| Topic | Dirección | Descripción |
|-------|-----------|-------------|
| `document-generate-request` | Consumer | Solicitud de generación |
| `document-batch-request` | Consumer | Procesamiento batch |
| `notification-dispatch-request` | Consumer | Envío de notificación |
| `document-generated-event` | Producer | Documento generado |
| `document-generation-failed-event` | Producer | Error de generación |
| `notification-status-event` | Producer | Estado de notificación |

---

## Generadores de Documentos

| Tipo | Generador | Formato | Tests |
|------|-----------|---------|-------|
| Factura Fiscal | `invoice_generator.rs` | PDF | ✅ |
| Reporte | `report_generator.rs` | PDF | ✅ |
| Excel | `excel_generator.rs` | XLSX | ✅ |
| CSV | `csv_generator.rs` | CSV | ✅ |
| QR Code | `qr_generator.rs` | PNG/Base64 | ✅ |
| Cotización | `quotation_generator.rs` | PDF | ✅ |

---

## Notificaciones

### WhatsApp (EvolutionAPI)
- **Instancia**: FACTURAZO-ERP-DEV
- **Servidor**: http://5.161.120.166:8080
- **Validación**: Teléfonos dominicanos (+1809, +1829, +1849)
- **Tests**: `test_normalize_dominican_phone`, `test_validate_dominican_phone`

### Email (SMTP)
- Cliente: lettre
- Templates HTML
- Soporte attachments

---

## Contexto de Negocio

### República Dominicana
- **NCF**: Número de Comprobante Fiscal
- **ITBIS**: 18% impuesto
- **Retención**: 7 años (requerimiento legal)
- **Formatos**: B01, B02, B14, B15, etc.

### Clientes
- **Le Croissant Doré**: 80 sucursales, alto volumen
- **Facturazo**: Multi-tenant, white-label

---

## Performance Targets

| Métrica | Objetivo |
|---------|----------|
| Disponibilidad | 99.9% |
| Latencia Sync P99 | < 500ms |
| Latencia Async P99 | < 5s |
| Throughput Docs | 10,000/hora |
| Throughput Notif | 50,000/hora |
| Error Rate | < 0.1% |

---

## Comandos

```bash
# Compilar
cargo build --release

# Ejecutar API
cargo run --bin pdf-services

# Ejecutar Worker Kafka
cargo run --bin kafka_worker

# Ejecutar tests
cargo test --lib

# Benchmark
cargo run --bin benchmark-report
```

---

## Variables de Entorno

```env
# Database
DATABASE_URL=sqlite://data/documents.db

# Kafka
KAFKA_BROKERS=127.0.0.1:9092

# Storage
S3_ENDPOINT=http://127.0.0.1:9000
S3_BUCKET=documents

# API
API_PORT=8080
RATE_LIMIT_REQUESTS=100

# WhatsApp
EVOLUTION_API_URL=http://5.161.120.166:8080
EVOLUTION_API_KEY=your-api-key
EVOLUTION_INSTANCE=FACTURAZO-ERP-DEV

# Email
SMTP_HOST=smtp.example.com
SMTP_USER=user
SMTP_PASSWORD=password
```

---

*Última actualización: 2024-11-24*
