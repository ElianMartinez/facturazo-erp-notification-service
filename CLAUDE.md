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