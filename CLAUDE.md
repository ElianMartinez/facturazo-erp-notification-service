# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Comandos de desarrollo

### Compilación y ejecución
- `cargo build --release` - Compilar el proyecto en modo release
- `cargo run --bin pdf-services` - Ejecutar el generador de facturas fiscales
- `cargo run --bin benchmark-report` - Ejecutar el generador de reportes de benchmark
- `typst compile archivo.typ archivo.pdf` - Compilar archivos Typst a PDF

### Binarios generados
- `./target/release/pdf-services` - Generador de facturas fiscales electrónicas
- `./target/release/benchmark-report` - Generador de reportes de facturación con benchmark

## Arquitectura del proyecto

Este proyecto en Rust genera documentos PDF usando Typst como motor de renderizado. La arquitectura consta de:

### Flujo de generación de documentos
1. **Generación de datos**: Los binarios de Rust generan contenido dinámico (QR codes, datos de facturas)
2. **Plantillas Typst**: Se crean archivos `.typ` temporales con formato de documento
3. **Compilación**: Se usa el comando `typst` del sistema para compilar a PDF
4. **Limpieza**: Se eliminan archivos temporales después de la generación

### Componentes principales

#### pdf-services (main.rs)
- Genera facturas fiscales electrónicas dominicanas con código QR
- Usa la biblioteca `qrcode` para generar QR codes como imágenes PNG
- Crea plantillas Typst con diseño de factura fiscal incluyendo marca de agua "PAID"
- Los archivos se guardan en el directorio `facturas/`

#### benchmark-report (benchmark_report.rs)
- Herramienta de benchmark para generar reportes de facturación masivos
- Prueba rendimiento con diferentes cantidades de filas (5000, 10000, 20000)
- Genera reportes tabulares complejos con resúmenes ejecutivos
- Mide tiempo de generación y tamaño de archivo resultante
- Los reportes se guardan en el directorio `reportes/`

### Dependencias clave
- **qrcode + image**: Generación de códigos QR para facturas fiscales
- **chrono**: Manejo de fechas y timestamps
- **base64**: Codificación de datos (disponible pero no usada actualmente)
- **Typst** (externo): Motor de composición tipográfica instalado en el sistema

### Estructura de directorios generados
- `facturas/`: Facturas fiscales generadas
- `reportes/`: Reportes de benchmark generados

## Concurrency Control (Rate Limiting)

El servicio incluye un sistema de control de concurrencia adaptativo que previene sobrecarga del servidor.

### Variables de entorno

| Variable | Default | Descripción |
|----------|---------|-------------|
| `CONCURRENCY_MAX_PDF` | 2 | Máximo de reportes PDF simultáneos (Typst, más intensivo) |
| `CONCURRENCY_MAX_EXCEL` | 4 | Máximo de reportes Excel simultáneos |
| `CONCURRENCY_MAX_CSV` | 8 | Máximo de reportes CSV simultáneos |
| `CONCURRENCY_MAX_TOTAL` | 6 | Máximo total de reportes simultáneos |
| `CONCURRENCY_RAM_THRESHOLD` | 85 | % de RAM para rechazar nuevos jobs |
| `CONCURRENCY_LOAD_THRESHOLD` | 4.0 | Load average máximo (1-min) |
| `CONCURRENCY_MIN_FREE_RAM_MB` | 1024 | RAM mínima libre para jobs grandes |
| `CONCURRENCY_ADAPTIVE_ENABLED` | true | Habilitar control adaptativo |
| `CONCURRENCY_JOB_TIMEOUT_SECS` | 300 | Timeout por job (5 minutos) |

### Clasificación de Jobs

| Tipo | Registros | RAM Estimada |
|------|-----------|--------------|
| Small | < 500 | ~256 MB |
| Medium | 500-5000 | ~768 MB |
| Large | > 5000 | ~2 GB |

### Endpoint de métricas

```bash
# Ver estado del controlador de concurrencia
curl http://localhost:8080/concurrency
```

Respuesta de ejemplo:
```json
{
  "healthy": true,
  "resources": {
    "ram_percent": 45,
    "ram_free_mb": 4096,
    "load_1min": 1.2,
    "health_score": 78
  },
  "jobs": {
    "accepted": 150,
    "rejected": 2,
    "completed": 148,
    "active": 2
  },
  "semaphore": {
    "active_pdf": 1,
    "active_excel": 1,
    "queued_pdf": 0
  }
}
```

### Comportamiento bajo carga

1. **RAM > 85%** o **Load > 4.0**: Rechaza nuevos jobs con HTTP 503
2. **RAM > 68%** o **Load > 3.2**: Aplica throttling (delay 1s)
3. **Jobs grandes (>5000 registros)**: Requieren >= 1GB RAM libre
4. **Cola llena**: Rechaza con HTTP 503 si hay más de 100 jobs encolados

## CI/CD Pipeline

El proyecto usa GitHub Actions con self-hosted runners.

### Jobs del pipeline
| Evento | Build | Lint | Test | Security | Docker | Deploy |
|--------|-------|------|------|----------|--------|--------|
| PR a main | ✓ | ✓ | ✓ | ✓ | ❌ | ❌ |
| Push a main | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Release | ✓ | ✓ | ✓ | ✓ | ✓ (prod) | ❌ |

### Docker tags
- `dev-latest` - Push a main
- `prod-latest` - Release publicado
