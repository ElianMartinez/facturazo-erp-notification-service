# PROJECT CONTEXT: Document & Notification Service

## 🎯 Objetivo del Proyecto
Crear un microservicio robusto de nivel enterprise que maneje la generación de documentos (PDF, Excel, CSV) y el envío de notificaciones (Email, WhatsApp, In-App) de manera asíncrona y escalable.

## 🏗️ Arquitectura General

### Stack Tecnológico
- **Lenguaje**: Rust (performance y seguridad)
- **Runtime Async**: Tokio
- **Message Bus**: Kafka (procesamiento asíncrono)
- **API**: Actix-web (HTTP)
- **Generación PDF**: Typst (moderno, rápido)
- **Generación Excel**: rust_xlsxwriter
- **Storage**: Cloudflare R2 (S3-compatible)
- **Database**: SQLite (con SQLx)
- **Cache**: In-memory (dashmap/moka)
- **Notificaciones**:
  - **WhatsApp**: EvolutionAPI
  - **Email**: SMTP propio
- **Observability**: OpenTelemetry + Prometheus

### Arquitectura de Capas

```
┌─────────────────────────────────────────────────────────────┐
│                   Document & Notification Service            │
├─────────────────────────────────────────────────────────────┤
│  API Layer      → HTTP/gRPC endpoints                        │
│  Message Bus    → Kafka consumers/producers                  │
│  Application    → Use cases, commands, orchestrators         │
│  Domain        → Business logic, entities, value objects     │
│  Infrastructure → External services, storage, notifications  │
└─────────────────────────────────────────────────────────────┘
```

## 📋 Componentes Principales

### 1. Document Generation
- **PDF**: Usando Typst con templates versionados
- **Excel**: rust_xlsxwriter para reportes complejos
- **CSV**: Para exportación de datos masivos
- **Templates**: Sistema de plantillas versionadas y cacheadas

### 2. Notification System
- **Email**: SMTP propio con templates HTML
- **WhatsApp**: EvolutionAPI con mensajes template
- **In-App**: WebSocket/Server-Sent Events
- **SMS**: Fallback para WhatsApp (opcional)

### 3. Storage System
- **Cloudflare R2**: Almacenamiento principal de documentos
- **SQLite**: Metadata y estado de documentos (embedded database)
- **In-Memory Cache**: Cache de templates y URLs firmadas (dashmap/moka)
- **Retention**: 7 años para facturas (requerimiento legal RD)

### 4. Message Processing
- **Kafka Topics**:
  - `document-generate-request`: Solicitudes de generación
  - `document-batch-request`: Procesamiento por lotes
  - `notification-dispatch-request`: Envío de notificaciones
  - `document-generated-event`: Eventos de éxito
  - `document-generation-failed-event`: Eventos de error
  - `notification-status-event`: Estado de notificaciones

### 5. Observability
- **Logging**: Structured logging con tracing
- **Metrics**: Prometheus para métricas de negocio
- **Tracing**: OpenTelemetry para distributed tracing
- **Health Checks**: Liveness y Readiness probes

## 🔐 Seguridad y Compliance

### Requerimientos
- **Encriptación**: En tránsito (TLS) y en reposo (AES-256)
- **Autenticación**: JWT para API, SASL para Kafka
- **Autorización**: RBAC por tenant
- **Compliance**: GDPR, retención legal de 7 años
- **Rate Limiting**: Por tenant y endpoint
- **Circuit Breaker**: Para servicios externos

## 📊 Performance Targets

### SLOs (Service Level Objectives)
- **Disponibilidad**: 99.9% uptime
- **Latencia P99**:
  - Sync generation: < 500ms
  - Async generation: < 5s
- **Throughput**:
  - 10,000 documentos/hora
  - 50,000 notificaciones/hora
- **Error Rate**: < 0.1%

## 🔄 Flujo de Trabajo Principal

### Generación de Documento
1. Request llega vía API o Kafka
2. Validación y autorización
3. Selección de template y motor
4. Generación del documento
5. Upload a R2
6. Persistencia de metadata
7. Trigger de notificación
8. Emisión de evento de éxito

### Envío de Notificación
1. Evento de documento generado
2. Resolución de canales y destinatarios
3. Preparación de contenido por canal
4. Envío paralelo a múltiples canales
5. Manejo de reintentos y fallbacks
6. Tracking de estado y métricas

## 🏢 Multi-Tenancy

### Estrategia
- **Aislamiento**: Por tenant_id en todos los niveles
- **Quotas**: Límites por tenant configurables
- **Storage**: Paths separados en R2
- **Database**: SQLite con tablas multi-tenant (tenant_id column)
- **Cache**: In-memory cache con namespace por tenant

## 🚀 Deployment

### Kubernetes
- **Pods**: Auto-scaling horizontal (HPA)
- **ConfigMaps**: Configuración de templates
- **Secrets**: Credenciales de servicios
- **Ingress**: NGINX con rate limiting
- **Service Mesh**: Istio para observability

### CI/CD
- **Build**: GitHub Actions
- **Registry**: GitHub Container Registry
- **Deployment**: ArgoCD (GitOps)
- **Environments**: dev, staging, production

## 📝 Notas de Contexto

### Decisiones de Diseño
1. **Rust sobre Go/Java**: Por performance crítica en generación de PDFs
2. **Typst sobre LaTeX**: Más moderno, rápido y fácil de mantener
3. **Kafka sobre RabbitMQ**: Por throughput y durabilidad
4. **R2 sobre S3**: Mejor pricing para egress
5. **Actix-web**: Framework maduro, estable y bien conocido
6. **SQLite sobre PostgreSQL**: Simplicidad, embedded, zero-config
7. **In-Memory Cache sobre Redis**: Menos complejidad, menor latencia
8. **EvolutionAPI**: Control total sobre WhatsApp Business

### Consideraciones Especiales
- **República Dominicana**: NCF, ITBIS, retención 7 años
- **Le Croissant Doré**: 80 sucursales, alto volumen
- **Facturazo**: Multi-tenant, white-label ready
- **Performance**: Generación batch crítica (fin de mes)

## 🔗 Enlaces y Recursos

### Documentación Externa
- [Typst Documentation](https://typst.app/docs)
- [Cloudflare R2 API](https://developers.cloudflare.com/r2/)
- [Kafka Best Practices](https://kafka.apache.org/documentation/#bestpractices)
- [EvolutionAPI Docs](https://doc.evolution-api.com/)
- [Actix-web Guide](https://actix.rs/docs/)

### Repositorios Relacionados
- Frontend: `facturazo-web`
- Mobile: `facturazo-mobile`
- Legacy System: `pdf-services` (current)

---
*Última actualización: 2024-11-24*
*Autor: Augusto (Claude)*
*Version: 1.0.0*