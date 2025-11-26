# PDF Services - API & Kafka Integration Documentation

## Table of Contents
1. [Overview](#overview)
2. [Authentication](#authentication)
3. [REST API Endpoints](#rest-api-endpoints)
4. [Kafka Integration](#kafka-integration)
5. [Data Models](#data-models)
6. [Examples](#examples)
7. [Environment Configuration](#environment-configuration)

---

## Overview

PDF Services is a microservice for generating documents (PDF, Excel, CSV) and sending notifications (WhatsApp, Email). It supports both synchronous REST API calls and asynchronous Kafka message processing.

### Base URL
```
Production: http://5.161.120.166:8980
```

### Supported Features
- **Document Generation**: Invoice, Report, Receipt, Quotation, Custom
- **Output Formats**: PDF, Excel (XLSX), CSV, HTML
- **Notification Channels**: WhatsApp (via EvolutionAPI), Email (SMTP)
- **Delivery Options**: Sync API, Async API, Kafka events

---

## Authentication

All `/api/v1/*` endpoints require Bearer token authentication.

### Token Format
```
Authorization: Bearer valid_tenant{ID}_user{ID}
```

### Example
```bash
curl -H "Authorization: Bearer valid_tenant1_user1" ...
```

> **Note**: In production, replace with JWT validation. The current implementation accepts tokens starting with `valid_` for development.

---

## REST API Endpoints

### Health Check Endpoints

#### GET /health
Check service health status.

```bash
curl http://localhost:8980/health
```

**Response:**
```json
{
  "status": "healthy"
}
```

#### GET /ready
Check readiness (S3, templates loaded).

```bash
curl http://localhost:8980/ready
```

**Response:**
```json
{
  "status": "ready",
  "checks": {
    "s3": "ok",
    "templates": "ok"
  }
}
```

---

### Document Generation Endpoints

#### POST /api/v1/documents/generate/sync
Generate document synchronously. Best for small documents (<1MB).

**Headers:**
```
Authorization: Bearer valid_tenant1_user1
Content-Type: application/json
```

**Request Body:**
```json
{
  "template_id": "invoice_fiscal",
  "document_type": "invoice",
  "priority": "normal",
  "format": "pdf",
  "data": {
    "company": {
      "name": "Mi Empresa SRL",
      "rnc": "123456789",
      "address": "Santo Domingo, DN",
      "phone": "809-555-1234"
    },
    "client": {
      "name": "Cliente SA",
      "rnc": "987654321",
      "address": "Santiago, RD"
    },
    "invoice_number": "B0100000001",
    "date": "2025-11-26",
    "items": [
      {
        "description": "Servicio de consultoría",
        "quantity": 1,
        "unit_price": 5000.00,
        "total": 5000.00
      }
    ],
    "subtotal": 5000.00,
    "itbis": 900.00,
    "total": 5900.00
  },
  "metadata": {
    "tenant_id": 1,
    "user_id": 1,
    "ttl_seconds": 86400
  },
  "notification": {
    "whatsapp": {
      "phone": "18296630497",
      "message": "Tu factura está lista para descargar."
    },
    "email": {
      "to": "cliente@email.com",
      "subject": "Tu factura está lista",
      "body": "Adjunto encontrarás tu factura."
    }
  }
}
```

**Response (200 OK):**
```json
{
  "id": "06af92ee-a4a5-4bfc-95f7-4a902fe4994c",
  "status": "completed",
  "url": "facturazo-documents/1/invoice/71d878bc.pdf",
  "error": null,
  "processing_time_ms": 979,
  "created_at": "2025-11-26T17:25:58.327129001Z",
  "expires_at": null
}
```

---

#### POST /api/v1/documents/generate/async
Queue document for asynchronous generation. Best for large documents or batch processing.

**Request Body:** Same as sync endpoint.

**Response (202 Accepted):**
```json
{
  "id": "06af92ee-a4a5-4bfc-95f7-4a902fe4994c",
  "status": "processing",
  "estimated_time_seconds": 30,
  "status_url": "/api/v1/documents/06af92ee-a4a5-4bfc-95f7-4a902fe4994c/status"
}
```

---

#### GET /api/v1/documents/{id}/status
Get document generation status.

**Response:**
```json
{
  "status": "completed",
  "message": "Document ready for download"
}
```

---

#### GET /api/v1/documents/{id}/download
Download generated document (redirects to presigned S3 URL).

---

#### POST /api/v1/documents/upload
Upload large data files for batch processing.

**Headers:**
```
Content-Type: application/json
Content-Encoding: gzip  (optional)
```

**Response:**
```json
{
  "status": "uploaded",
  "data_reference": {
    "bucket": "temp-uploads",
    "key": "uploads/1/abc123.json",
    "expires_in": 86400
  }
}
```

---

### Template Endpoints

#### GET /api/v1/templates/list
List available templates.

**Response:**
```json
{
  "templates": [
    {
      "id": "fiscal_electronic",
      "category": "invoice",
      "path": "invoice/fiscal_electronic",
      "description": "Factura fiscal electrónica dominicana"
    }
  ]
}
```

---

#### POST /api/v1/templates/generate
Generate PDF from template with custom data.

**Request Body:**
```json
{
  "template_id": "fiscal_electronic",
  "template_type": "invoice",
  "data": {
    "invoice_number": "INV-2024-001",
    "issue_date": "2024-01-15",
    "due_date": "2024-02-15",
    "company_info": { ... },
    "client_info": { ... },
    "items": [ ... ],
    "totals": { ... },
    "fiscal_info": { ... }
  },
  "output_filename": "mi_factura"
}
```

---

#### GET /api/v1/templates/preview/{template_id}
Preview template with sample data (returns PDF binary).

---

## Kafka Integration

### Topics

| Topic | Purpose | Consumer Group |
|-------|---------|----------------|
| `document-generate-request` | Single document generation | `document-service` |
| `document-batch-request` | Batch document processing | `document-service` |
| `notification-dispatch-request` | Notification delivery | `document-service` |
| `document-events` | Output events (results, errors) | Your service |
| `document-dlq` | Dead letter queue | Monitoring |

---

### Message Types

All Kafka messages use JSON format with a `type` field for routing:

#### 1. Generate Document
```json
{
  "type": "generate_document",
  "request_id": "req-123",
  "tenant_id": "tenant-1",
  "user_id": "user-1",
  "document_type": "invoice",
  "format": "pdf",
  "template_id": "invoice_fiscal",
  "data": {
    "company": { ... },
    "client": { ... },
    "items": [ ... ]
  },
  "store": true,
  "callback_url": "https://your-service.com/webhook"
}
```

**Document Types:** `invoice`, `quotation`, `report`, `receipt`, `credit_note`, `custom`
**Formats:** `pdf`, `excel`, `xlsx`, `csv`, `html`

---

#### 2. Send Notification
```json
{
  "type": "send_notification",
  "request_id": "notif-456",
  "tenant_id": "tenant-1",
  "channel": "whatsapp",
  "recipient": "18296630497",
  "subject": null,
  "message": "Tu documento está listo!",
  "document_id": "doc-789",
  "callback_url": "https://your-service.com/webhook"
}
```

**Channels:** `email`, `whatsapp`, `wa`, `sms`

---

#### 3. Generate and Notify
Generate document and send notification in one operation.

```json
{
  "type": "generate_and_notify",
  "request_id": "gen-notif-001",
  "tenant_id": "tenant-1",
  "document": {
    "request_id": "doc-001",
    "tenant_id": "tenant-1",
    "document_type": "invoice",
    "format": "pdf",
    "template_id": "invoice_fiscal",
    "data": { ... },
    "store": true
  },
  "notification": {
    "request_id": "notif-001",
    "tenant_id": "tenant-1",
    "channel": "whatsapp",
    "recipient": "18296630497",
    "message": "Tu factura está lista!"
  }
}
```

---

#### 4. Batch Generate
Generate multiple documents in one request.

```json
{
  "type": "batch_generate",
  "request_id": "batch-001",
  "tenant_id": "tenant-1",
  "batch_id": "monthly-invoices-2025-11",
  "documents": [
    {
      "request_id": "inv-001",
      "tenant_id": "tenant-1",
      "document_type": "invoice",
      "format": "pdf",
      "template_id": "invoice_fiscal",
      "data": { ... },
      "store": true
    },
    {
      "request_id": "inv-002",
      "tenant_id": "tenant-1",
      "document_type": "invoice",
      "format": "pdf",
      "template_id": "invoice_fiscal",
      "data": { ... },
      "store": true
    }
  ],
  "parallel": true,
  "callback_url": "https://your-service.com/batch-webhook"
}
```

---

#### 5. Batch Notify (Multiple Recipients)
Send same notification to multiple recipients.

```json
{
  "type": "batch_notify",
  "request_id": "batch-notif-001",
  "tenant_id": "tenant-1",
  "batch_id": "promo-campaign-001",
  "channel": "whatsapp",
  "recipients": [
    "18291234567",
    "18297654321",
    "18299876543"
  ],
  "subject": null,
  "message": "Aprovecha nuestra promoción especial!",
  "document_id": null,
  "parallel": true,
  "concurrency": 10,
  "callback_url": "https://your-service.com/webhook"
}
```

---

#### 6. Generate and Notify Many
Generate one document and send to multiple recipients.

```json
{
  "type": "generate_and_notify_many",
  "request_id": "gen-many-001",
  "tenant_id": "tenant-1",
  "document": {
    "request_id": "report-001",
    "tenant_id": "tenant-1",
    "document_type": "report",
    "format": "pdf",
    "template_id": "monthly_report",
    "data": { ... },
    "store": true
  },
  "notification": {
    "channel": "email",
    "recipients": [
      "manager@company.com",
      "director@company.com",
      "ceo@company.com"
    ],
    "subject": "Reporte Mensual - Noviembre 2025",
    "message": "Adjunto el reporte mensual.",
    "parallel": true,
    "concurrency": 5
  }
}
```

---

### Response Events

Results are published to `document-events` topic:

#### Document Generated
```json
{
  "type": "document_generated",
  "request_id": "req-123",
  "document_id": "doc-abc-123",
  "storage_url": "facturazo-documents/tenant-1/invoice/doc-abc-123.pdf",
  "file_size": 15234,
  "generation_time_ms": 245
}
```

#### Notification Sent
```json
{
  "type": "notification_sent",
  "request_id": "notif-456",
  "notification_id": "notif-xyz-789",
  "status": "Sent"
}
```

#### Batch Completed
```json
{
  "type": "batch_completed",
  "request_id": "batch-001",
  "batch_id": "monthly-invoices-2025-11",
  "total": 100,
  "successful": 98,
  "failed": 2
}
```

#### Batch Notify Completed
```json
{
  "type": "batch_notify_completed",
  "request_id": "batch-notif-001",
  "batch_id": "promo-campaign-001",
  "total": 50,
  "successful": 48,
  "failed": 2,
  "notification_ids": ["notif-1", "notif-2", ...]
}
```

---

## Data Models

### Document Types
| Type | Description |
|------|-------------|
| `invoice` | Facturas fiscales |
| `quotation` | Cotizaciones |
| `report` | Reportes (tablas, resúmenes) |
| `receipt` | Recibos |
| `credit_note` | Notas de crédito |
| `custom` | Documentos personalizados |

### Output Formats
| Format | Extension | MIME Type |
|--------|-----------|-----------|
| `pdf` | .pdf | application/pdf |
| `excel` | .xlsx | application/vnd.openxmlformats-officedocument.spreadsheetml.sheet |
| `csv` | .csv | text/csv |
| `html` | .html | text/html |

### Notification Channels
| Channel | Description | Requirements |
|---------|-------------|--------------|
| `email` | Email via SMTP | SMTP_HOST, SMTP_USER, SMTP_PASS |
| `whatsapp` | WhatsApp via EvolutionAPI | EVOLUTION_API_URL, EVOLUTION_API_KEY, EVOLUTION_INSTANCE |
| `sms` | SMS (not implemented) | - |

### Priority Levels
| Priority | Description |
|----------|-------------|
| `low` | Background processing |
| `normal` | Standard processing |
| `high` | Priority queue |
| `urgent` | Immediate processing |

---

## Examples

### Example 1: Generate Invoice and Send via WhatsApp (REST)

```bash
curl -X POST 'http://5.161.120.166:8980/api/v1/documents/generate/sync' \
  -H 'Authorization: Bearer valid_tenant1_user1' \
  -H 'Content-Type: application/json' \
  -d '{
    "template_id": "invoice_fiscal",
    "document_type": "invoice",
    "priority": "normal",
    "format": "pdf",
    "data": {
      "company": {
        "name": "FACTURAZO SRL",
        "rnc": "123456789",
        "address": "Santo Domingo, DN"
      },
      "client": {
        "name": "Cliente SA",
        "rnc": "987654321"
      },
      "invoice_number": "B0100000001",
      "date": "2025-11-26",
      "items": [
        {"description": "Producto A", "quantity": 2, "unit_price": 1000, "total": 2000}
      ],
      "subtotal": 2000,
      "itbis": 360,
      "total": 2360
    },
    "metadata": {},
    "notification": {
      "whatsapp": {
        "phone": "18296630497",
        "message": "Tu factura B0100000001 está lista."
      }
    }
  }'
```

---

### Example 2: Generate Excel Report (Kafka)

**Publish to `document-generate-request`:**
```json
{
  "type": "generate_document",
  "request_id": "excel-report-001",
  "tenant_id": "tenant-1",
  "document_type": "report",
  "format": "excel",
  "template_id": "sales_report",
  "data": {
    "title": "Reporte de Ventas - Noviembre 2025",
    "headers": ["Fecha", "Producto", "Cantidad", "Total"],
    "rows": [
      ["2025-11-01", "Producto A", 10, 10000],
      ["2025-11-02", "Producto B", 5, 7500],
      ["2025-11-03", "Producto C", 20, 15000]
    ],
    "summary": {
      "total_ventas": 32500,
      "total_productos": 35
    }
  },
  "store": true
}
```

---

### Example 3: Send WhatsApp Notification Only (Kafka)

**Publish to `notification-dispatch-request`:**
```json
{
  "type": "send_notification",
  "request_id": "wa-notif-001",
  "tenant_id": "tenant-1",
  "channel": "whatsapp",
  "recipient": "18296630497",
  "message": "Hola! Tu pedido #12345 ha sido enviado.",
  "document_id": null
}
```

---

### Example 4: Mass Notification Campaign (Kafka)

**Publish to `notification-dispatch-request`:**
```json
{
  "type": "batch_notify",
  "request_id": "campaign-001",
  "tenant_id": "tenant-1",
  "batch_id": "black-friday-2025",
  "channel": "whatsapp",
  "recipients": [
    "18291111111",
    "18292222222",
    "18293333333"
  ],
  "message": "Black Friday! 50% de descuento en toda la tienda.",
  "parallel": true,
  "concurrency": 20
}
```

---

### Example 5: Generate Invoice with Email (REST)

```bash
curl -X POST 'http://5.161.120.166:8980/api/v1/documents/generate/sync' \
  -H 'Authorization: Bearer valid_tenant1_user1' \
  -H 'Content-Type: application/json' \
  -d '{
    "template_id": "invoice_fiscal",
    "document_type": "invoice",
    "format": "pdf",
    "data": { ... },
    "notification": {
      "email": {
        "to": "cliente@email.com",
        "subject": "Factura #B0100000001",
        "body": "Estimado cliente, adjunto su factura."
      }
    }
  }'
```

---

## Environment Configuration

### Required Variables

```bash
# Server
HOST=0.0.0.0
PORT=8980

# AWS S3 / Cloudflare R2
AWS_ACCESS_KEY_ID=your-access-key
AWS_SECRET_ACCESS_KEY=your-secret-key
AWS_REGION=us-east-1
S3_BUCKET_DOCUMENTS=documents
S3_BUCKET_TEMP=temp-uploads

# Email (SMTP)
SMTP_HOST=smtp.gmail.com
SMTP_PORT=587
SMTP_USER=your-email@gmail.com
SMTP_PASS=your-app-password
SMTP_FROM_EMAIL=noreply@yourdomain.com
SMTP_FROM_NAME=Your Company

# WhatsApp (EvolutionAPI)
EVOLUTION_API_URL=https://your-evolution-api.com
EVOLUTION_API_KEY=your-api-key
EVOLUTION_INSTANCE=your-instance-name
```

### Kafka Configuration

```bash
# Kafka
KAFKA_BROKERS=localhost:9092
KAFKA_GROUP_ID=document-service
KAFKA_CLIENT_ID=document-service-01

# Topics
KAFKA_TOPIC_DOCUMENT_REQUEST=document-generate-request
KAFKA_TOPIC_DOCUMENT_BATCH=document-batch-request
KAFKA_TOPIC_NOTIFICATION_DISPATCH=notification-dispatch-request
KAFKA_TOPIC_EVENTS=document-events
KAFKA_TOPIC_DLQ=document-dlq
```

---

## Integration from Core-Service

### Kotlin/Java Example (REST)

```kotlin
val client = HttpClient.newHttpClient()
val request = HttpRequest.newBuilder()
    .uri(URI.create("http://pdf-service:8980/api/v1/documents/generate/sync"))
    .header("Authorization", "Bearer valid_tenant1_user1")
    .header("Content-Type", "application/json")
    .POST(HttpRequest.BodyPublishers.ofString("""
        {
            "template_id": "invoice_fiscal",
            "document_type": "invoice",
            "format": "pdf",
            "data": $invoiceJson,
            "notification": {
                "whatsapp": {
                    "phone": "$customerPhone",
                    "message": "Tu factura está lista"
                }
            }
        }
    """))
    .build()

val response = client.send(request, HttpResponse.BodyHandlers.ofString())
```

### Kotlin/Java Example (Kafka)

```kotlin
@Service
class DocumentService(
    private val kafkaTemplate: KafkaTemplate<String, String>
) {
    fun generateInvoice(invoice: Invoice, phone: String) {
        val message = mapOf(
            "type" to "generate_and_notify",
            "request_id" to UUID.randomUUID().toString(),
            "tenant_id" to invoice.tenantId,
            "document" to mapOf(
                "request_id" to UUID.randomUUID().toString(),
                "tenant_id" to invoice.tenantId,
                "document_type" to "invoice",
                "format" to "pdf",
                "template_id" to "invoice_fiscal",
                "data" to invoice.toJson(),
                "store" to true
            ),
            "notification" to mapOf(
                "request_id" to UUID.randomUUID().toString(),
                "tenant_id" to invoice.tenantId,
                "channel" to "whatsapp",
                "recipient" to phone,
                "message" to "Tu factura ${invoice.number} está lista."
            )
        )

        kafkaTemplate.send("document-generate-request", objectMapper.writeValueAsString(message))
    }
}
```

---

## Error Handling

### HTTP Status Codes

| Code | Description |
|------|-------------|
| 200 | Success |
| 202 | Accepted (async processing) |
| 400 | Bad Request (invalid data) |
| 401 | Unauthorized (missing/invalid token) |
| 413 | Payload Too Large |
| 429 | Too Many Requests (rate limited) |
| 500 | Internal Server Error |

### Rate Limiting

- Default: 100 requests/minute per tenant:user
- Burst: 20 requests
- Returns 429 with `retry_after: 60`

---

## Support

For issues or questions:
- GitHub: https://github.com/your-repo/pdf-services
- Documentation: This file
