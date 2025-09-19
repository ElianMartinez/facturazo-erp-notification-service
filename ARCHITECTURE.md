# Arquitectura del Sistema de Generación de Documentos

## Estructura del Proyecto

```
pdf-services/
├── src/
│   ├── api/                    # API REST con Actix-web
│   │   ├── handlers.rs         # Manejadores de endpoints
│   │   ├── middleware/         # Middleware de autenticación y compresión
│   │   ├── routes.rs           # Definición de rutas
│   │   ├── state.rs            # Estado compartido de la API
│   │   └── template_handler.rs # Manejador específico para templates
│   │
│   ├── generators/             # Generadores de documentos
│   │   ├── pdf.rs              # Generador de PDFs con Typst
│   │   └── excel.rs            # Generador de Excel con rust_xlsxwriter
│   │
│   ├── models/                 # Modelos de datos
│   │   ├── document.rs         # Modelo de documento genérico
│   │   ├── invoice.rs          # Modelo de factura
│   │   ├── report.rs           # Modelo de reporte
│   │   └── common.rs           # Tipos comunes compartidos
│   │
│   ├── storage/                # Almacenamiento en la nube
│   │   └── s3.rs               # Cliente S3 para almacenamiento
│   │
│   ├── templates/              # Sistema de plantillas dinámicas
│   │   ├── template_engine.rs  # Motor de procesamiento de templates
│   │   ├── template_models.rs  # Modelos de datos para templates
│   │   ├── template_trait.rs   # Trait base y registro de templates
│   │   └── templates/          # Plantillas dinámicas en Rust
│   │       ├── fiscal_invoice.rs   # Factura fiscal electrónica
│   │       ├── simple_invoice.rs   # Factura simple
│   │       ├── receipt.rs          # Recibo de pago
│   │       └── report.rs           # Reporte genérico
│   │
│   ├── main.rs                 # Entrada principal (API server)
│   ├── worker.rs               # Worker para procesamiento asíncrono
│   └── lib.rs                  # Biblioteca principal
│
├── output/                     # PDFs generados (gitignored)
├── facturas/                   # Facturas generadas (gitignored)
├── Cargo.toml                  # Dependencias de Rust
├── .env                        # Variables de entorno (gitignored)
└── README.md                   # Documentación principal
```

## Componentes Principales

### 1. API REST (`src/api/`)
- **Servidor HTTP**: Actix-web
- **Autenticación**: JWT con middleware personalizado
- **Rate Limiting**: Governor con límites por tenant/usuario
- **Endpoints principales**:
  - `POST /api/v1/generate/sync` - Generación síncrona
  - `POST /api/v1/generate/async` - Generación asíncrona
  - `GET /api/v1/documents/{id}` - Estado del documento
  - `POST /api/v1/templates/generate` - Generación con templates

### 2. Generadores (`src/generators/`)
- **PDF Generator**: Genera PDFs usando Typst como motor
- **Excel Generator**: Genera archivos Excel con rust_xlsxwriter
- Soporte para compresión (Gzip, Zstd)
- Generación de códigos QR para facturas fiscales

### 3. Sistema de Templates (`src/templates/`)
- **Templates Dinámicos**: Cada plantilla es un módulo Rust
- **Sin archivos .typ externos**: Todo el contenido se genera en código
- **Plantillas disponibles**:
  - Factura Fiscal Electrónica (República Dominicana)
  - Factura Simple
  - Recibo de Pago
  - Reporte con tablas y gráficos

### 4. Almacenamiento (`src/storage/`)
- **S3 Compatible**: MinIO, AWS S3, DigitalOcean Spaces
- **Multipart Upload**: Para archivos grandes
- **URLs firmadas**: Acceso temporal seguro

### 5. Procesamiento Asíncrono
- **Kafka**: Cola de mensajes para trabajos pesados
- **Worker**: Procesa documentos en background
- **Redis**: Cache y estado compartido

## Flujo de Generación de Documentos

1. **Request llega a la API** → Validación y autenticación
2. **Verificación de Rate Limit** → Por tenant y usuario
3. **Decisión Sync/Async**:
   - **Sync** (< 1MB): Genera y retorna inmediatamente
   - **Async** (> 1MB): Envía a Kafka, retorna ID
4. **Generación**:
   - Selecciona plantilla según tipo
   - Genera contenido Typst dinámicamente
   - Compila a PDF con comando `typst`
5. **Almacenamiento**: S3 con URL firmada
6. **Respuesta**: URL del documento o ID para polling

## Tecnologías Clave

- **Rust**: Lenguaje principal
- **Actix-web**: Framework web async
- **Typst**: Motor de composición tipográfica para PDFs
- **SQLx**: Acceso a base de datos (SQLite/PostgreSQL)
- **Redis**: Cache distribuido
- **Kafka**: Cola de mensajes
- **S3**: Almacenamiento de objetos

## Agregar Nueva Plantilla

1. Crear archivo en `src/templates/templates/nueva_plantilla.rs`
2. Implementar el trait `TypstTemplate`:
```rust
pub struct MiPlantilla;

impl TypstTemplate for MiPlantilla {
    fn generate(&self, data: &Value) -> Result<String> { ... }
    fn template_id(&self) -> &str { "mi_plantilla" }
    fn validate(&self, data: &Value) -> Result<()> { ... }
}
```
3. Registrar en `src/templates/template_trait.rs`
4. ¡Listo! Ya está disponible para usar

## Variables de Entorno

```env
DATABASE_URL=sqlite://data/documents.db
REDIS_URL=redis://127.0.0.1:6379
KAFKA_BROKERS=127.0.0.1:9092
S3_ENDPOINT=http://127.0.0.1:9000
S3_BUCKET=documents
API_PORT=8080
RATE_LIMIT_REQUESTS=100
RATE_LIMIT_WINDOW_SECS=60
```

## Comandos Útiles

```bash
# Compilar en modo release
cargo build --release

# Ejecutar API
cargo run --bin api

# Ejecutar worker
cargo run --bin worker

# Generar PDF con Typst
typst compile archivo.typ archivo.pdf
```