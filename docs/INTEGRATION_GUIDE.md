# PDF Services - Integration Guide

Complete guide for integrating `core-service` with `pdf-services` via HTTP and Kafka.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [HTTP API Reference](#http-api-reference)
3. [Kafka Messages Reference](#kafka-messages-reference)
4. [Examples by Use Case](#examples-by-use-case)
5. [Authentication](#authentication)
6. [Error Handling](#error-handling)

---

## Architecture Overview

```
┌─────────────────┐         ┌──────────────────────────────────────────┐
│                 │         │           PDF Services                    │
│  core-service   │         │                                          │
│                 │         │  ┌─────────────┐    ┌─────────────────┐  │
│  ┌───────────┐  │  HTTP   │  │   Actix     │    │   Generators    │  │
│  │  Invoice  │──┼────────►│  │   Web API   │───►│  PDF/Excel/CSV  │  │
│  │  Module   │  │  sync   │  └─────────────┘    └─────────────────┘  │
│  └───────────┘  │         │         │                   │            │
│                 │         │         ▼                   ▼            │
│  ┌───────────┐  │  Kafka  │  ┌─────────────┐    ┌─────────────────┐  │
│  │  Report   │──┼────────►│  │   Kafka     │    │   Storage       │  │
│  │  Module   │  │  async  │  │   Consumer  │    │   (S3/R2)       │  │
│  └───────────┘  │         │  └─────────────┘    └─────────────────┘  │
│                 │         │         │                   │            │
│  ┌───────────┐  │         │         ▼                   ▼            │
│  │  Notif.   │  │         │  ┌─────────────────────────────────────┐ │
│  │  Module   │  │         │  │         Notifications              │ │
│  └───────────┘  │         │  │   Email (SMTP) / WhatsApp (Evol.)  │ │
│                 │         │  └─────────────────────────────────────┘ │
└─────────────────┘         └──────────────────────────────────────────┘
```

### When to Use HTTP vs Kafka

| Scenario | Protocol | Reason |
|----------|----------|--------|
| Single invoice PDF | HTTP sync | Fast response needed |
| Small report (<100KB) | HTTP sync | Immediate download |
| Large report (>100KB) | HTTP async or Kafka | Long processing time |
| Batch invoices | Kafka | Background processing |
| Send notification | Kafka | Fire-and-forget |
| Generate + Notify | Kafka | Workflow orchestration |

---

## HTTP API Reference

### Base URL

```
http://localhost:8080/api/v1
```

### Authentication Headers

```http
Authorization: Bearer <jwt_token>
X-Tenant-Id: 1
X-User-Id: 100
```

---

### 1. Generate Document (Sync)

**Endpoint:** `POST /api/v1/documents/generate/sync`

Best for: Small documents that need immediate response.

#### Request

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "template_id": "invoice_fiscal",
  "document_type": "invoice",
  "priority": "high",
  "format": "pdf",
  "callback_url": "https://core-service.com/webhooks/documents",
  "metadata": {
    "tenant_id": 1,
    "user_id": 100,
    "organization_id": "org-123",
    "ttl_seconds": 86400,
    "tags": {
      "department": "sales",
      "period": "2024-Q4"
    }
  },
  "data": {
    "seller": {
      "name": "Mi Empresa SRL",
      "rnc": "123456789",
      "address": "Calle Principal #123, Santo Domingo",
      "phone": "809-555-1234",
      "email": "ventas@miempresa.com"
    },
    "customer": {
      "name": "Cliente Final SA",
      "rnc": "987654321",
      "address": "Av. Secundaria #456, Santiago"
    },
    "invoice": {
      "number": "FAC-2024-001234",
      "ncf": "B0100000001",
      "date": "2024-11-24",
      "due_date": "2024-12-24",
      "currency": "DOP"
    },
    "items": [
      {
        "code": "PROD-001",
        "description": "Producto de ejemplo",
        "quantity": 10,
        "unit_price": 150.00,
        "discount": 0,
        "tax_rate": 18
      }
    ],
    "totals": {
      "subtotal": 1500.00,
      "discount": 0,
      "tax": 270.00,
      "total": 1770.00
    },
    "options": {
      "include_qr": true,
      "watermark": null,
      "locale": "es-DO"
    }
  }
}
```

#### Response (Success)

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "completed",
  "url": "https://storage.example.com/documents/550e8400.pdf?signed=...",
  "error": null,
  "processing_time_ms": 245,
  "created_at": "2024-11-24T10:30:00Z",
  "expires_at": "2024-11-25T10:30:00Z"
}
```

#### Response (Redirected to Async)

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "processing",
  "estimated_time_seconds": 30,
  "status_url": "/api/v1/documents/550e8400-e29b-41d4-a716-446655440000/status"
}
```

---

### 2. Generate Document (Async)

**Endpoint:** `POST /api/v1/documents/generate/async`

Best for: Large documents, batch processing.

#### Request

Same as sync endpoint.

#### Response

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "processing",
  "estimated_time_seconds": 30,
  "status_url": "/api/v1/documents/550e8400-e29b-41d4-a716-446655440000/status"
}
```

---

### 3. Check Document Status

**Endpoint:** `GET /api/v1/documents/{id}/status`

#### Response

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "completed",
  "progress": 100,
  "message": "Document generated successfully",
  "url": "https://storage.example.com/documents/550e8400.pdf",
  "updated_at": "2024-11-24T10:30:15Z"
}
```

**Status values:** `queued`, `processing`, `completed`, `failed`, `cancelled`

---

### 4. Download Document

**Endpoint:** `GET /api/v1/documents/{id}/download`

Returns a redirect (302) to the presigned S3 URL.

---

### 5. Upload Large Data

**Endpoint:** `POST /api/v1/documents/upload`

For documents with data > 1MB.

#### Request

- `Content-Type: application/json`
- `Content-Encoding: gzip` (optional, recommended)
- Body: JSON data (can be gzipped)

#### Response

```json
{
  "status": "uploaded",
  "data_reference": {
    "bucket": "temp-uploads",
    "key": "uploads/100/abc123.json",
    "expires_in": 86400
  }
}
```

Then use this reference in your document request:

```json
{
  "template_id": "large_report",
  "document_type": "report",
  "data_reference": {
    "bucket": "temp-uploads",
    "key": "uploads/100/abc123.json"
  }
}
```

---

### 6. Health Check

**Endpoint:** `GET /health`

```json
{
  "status": "healthy"
}
```

---

### 7. Readiness Check

**Endpoint:** `GET /ready`

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

## Kafka Messages Reference

### Topics

| Topic | Purpose |
|-------|---------|
| `document-generate-request` | Single document generation |
| `document-batch-request` | Batch document generation |
| `notification-dispatch-request` | Send notifications |
| `document-events` | Events/responses (consume this) |
| `document-dlq` | Dead letter queue for failures |

---

### Message Type 1: Generate Document

**Topic:** `document-generate-request`

```json
{
  "type": "generate_document",
  "request_id": "req-uuid-12345",
  "tenant_id": "tenant-1",
  "user_id": "user-100",
  "document_type": "invoice",
  "format": "pdf",
  "template_id": "invoice_fiscal",
  "data": {
    "seller": { "name": "Mi Empresa", "rnc": "123456789" },
    "customer": { "name": "Cliente", "rnc": "987654321" },
    "invoice": { "number": "FAC-001", "ncf": "B0100000001" },
    "items": [
      { "description": "Producto", "quantity": 1, "unit_price": 100 }
    ],
    "totals": { "subtotal": 100, "tax": 18, "total": 118 }
  },
  "store": true,
  "callback_url": "https://core-service.com/webhooks/documents"
}
```

**Document Types:** `invoice`, `quotation`, `report`, `receipt`, `credit_note`, `custom_name`

**Formats:** `pdf`, `excel`, `csv`, `html`

---

### Message Type 2: Send Notification

**Topic:** `notification-dispatch-request`

#### WhatsApp Notification

```json
{
  "type": "send_notification",
  "request_id": "notif-uuid-12345",
  "tenant_id": "tenant-1",
  "channel": "whatsapp",
  "recipient": "18296630497",
  "subject": null,
  "message": "Su factura FAC-001 esta lista. Monto: RD$ 1,770.00",
  "document_id": "doc-uuid-12345",
  "callback_url": "https://core-service.com/webhooks/notifications"
}
```

#### Email Notification

```json
{
  "type": "send_notification",
  "request_id": "notif-uuid-67890",
  "tenant_id": "tenant-1",
  "channel": "email",
  "recipient": "cliente@example.com",
  "subject": "Su factura FAC-001 esta lista",
  "message": "Adjunto encontrara su factura electronica.",
  "document_id": "doc-uuid-12345",
  "callback_url": null
}
```

**Channels:** `email`, `whatsapp`, `sms`

---

### Message Type 3: Generate and Notify

**Topic:** `document-generate-request`

Generate a document AND send it via notification in a single workflow.

```json
{
  "type": "generate_and_notify",
  "request_id": "workflow-uuid-12345",
  "tenant_id": "tenant-1",
  "document": {
    "request_id": "doc-uuid",
    "tenant_id": "tenant-1",
    "document_type": "invoice",
    "format": "pdf",
    "template_id": "invoice_fiscal",
    "data": {
      "seller": { "name": "Mi Empresa" },
      "customer": { "name": "Cliente", "phone": "18296630497" },
      "invoice": { "number": "FAC-001", "ncf": "B0100000001" },
      "items": [],
      "totals": { "total": 1770.00 }
    },
    "store": true
  },
  "notification": {
    "request_id": "notif-uuid",
    "tenant_id": "tenant-1",
    "channel": "whatsapp",
    "recipient": "18296630497",
    "message": "Su factura FAC-001 esta lista. Monto: RD$ 1,770.00"
  }
}
```

---

### Message Type 4: Batch Generate

**Topic:** `document-batch-request`

```json
{
  "type": "batch_generate",
  "request_id": "batch-uuid-12345",
  "tenant_id": "tenant-1",
  "batch_id": "monthly-invoices-2024-11",
  "documents": [
    {
      "request_id": "doc-1",
      "tenant_id": "tenant-1",
      "document_type": "invoice",
      "format": "pdf",
      "template_id": "invoice_fiscal",
      "data": { "invoice": { "number": "FAC-001" } },
      "store": true
    },
    {
      "request_id": "doc-2",
      "tenant_id": "tenant-1",
      "document_type": "invoice",
      "format": "pdf",
      "template_id": "invoice_fiscal",
      "data": { "invoice": { "number": "FAC-002" } },
      "store": true
    }
  ],
  "parallel": true,
  "callback_url": "https://core-service.com/webhooks/batch"
}
```

---

### Message Type 5: Batch Notify (Multiple Recipients)

**Topic:** `notification-dispatch-request`

Send the same message to multiple recipients efficiently.

```json
{
  "type": "batch_notify",
  "request_id": "batch-notif-001",
  "tenant_id": "1",
  "batch_id": "promo-black-friday",
  "channel": "whatsapp",
  "recipients": [
    "18296630497",
    "18095551234",
    "18295559876",
    "18495554321"
  ],
  "subject": null,
  "message": "Aprovecha nuestras ofertas de Black Friday! 50% de descuento en todos los productos.",
  "document_id": null,
  "parallel": true,
  "concurrency": 10,
  "callback_url": "https://core-service.com/webhooks/notifications"
}
```

**Parameters:**
- `recipients`: Array of phone numbers or emails
- `parallel`: Process in parallel (recommended: `true`)
- `concurrency`: Max simultaneous sends (default: 10, max recommended: 20)
- `document_id`: Optional - attach same document to all notifications

---

### Message Type 6: Generate and Notify Many

**Topic:** `document-generate-request`

Generate ONE document and send it to MULTIPLE recipients.

```json
{
  "type": "generate_and_notify_many",
  "request_id": "invoice-broadcast-001",
  "tenant_id": "1",
  "document": {
    "request_id": "doc-001",
    "tenant_id": "1",
    "document_type": "invoice",
    "format": "pdf",
    "template_id": "invoice_fiscal",
    "data": {
      "seller": { "name": "Mi Empresa" },
      "invoice": { "number": "FAC-001", "ncf": "B0100000001" },
      "totals": { "total": 1770.00 }
    },
    "store": true
  },
  "notification": {
    "channel": "whatsapp",
    "recipients": [
      "18296630497",
      "18095551234",
      "18295559876"
    ],
    "subject": null,
    "message": "Su factura FAC-001 esta lista. Total: RD$ 1,770.00",
    "parallel": true,
    "concurrency": 10
  }
}
```

**Use case:** Send the same invoice to multiple contacts (e.g., accounting department, customer, sales rep).

---

### Response Events

**Topic:** `document-events`

Listen to this topic to receive processing results.

#### Document Generated

```json
{
  "type": "document_generated",
  "request_id": "req-uuid-12345",
  "document_id": "doc-uuid-67890",
  "storage_url": "https://storage.example.com/documents/doc-uuid-67890.pdf",
  "file_size": 45678,
  "generation_time_ms": 234
}
```

#### Notification Sent

```json
{
  "type": "notification_sent",
  "request_id": "notif-uuid-12345",
  "notification_id": "3EB0345EB4EEFC90E376",
  "status": "Sent"
}
```

#### Batch Completed

```json
{
  "type": "batch_completed",
  "request_id": "batch-uuid-12345",
  "batch_id": "monthly-invoices-2024-11",
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
  "batch_id": "promo-black-friday",
  "total": 4,
  "successful": 4,
  "failed": 0,
  "notification_ids": [
    "3EB0345EB4EEFC90E376A001",
    "3EB0345EB4EEFC90E376A002",
    "3EB0345EB4EEFC90E376A003",
    "3EB0345EB4EEFC90E376A004"
  ]
}
```

#### Generate and Notify Many Completed

```json
{
  "type": "generate_and_notify_many_completed",
  "request_id": "invoice-broadcast-001",
  "document_id": "doc-uuid-12345",
  "total_recipients": 3,
  "notifications_sent": 3,
  "notifications_failed": 0
}
```

---

## Examples by Use Case

### Use Case 1: Generate Invoice and Send via WhatsApp

**Option A: HTTP (sync for small documents)**

```bash
# 1. Generate invoice
curl -X POST http://localhost:8080/api/v1/documents/generate/sync \
  -H "Content-Type: application/json" \
  -H "X-Tenant-Id: 1" \
  -d '{
    "template_id": "invoice_fiscal",
    "document_type": "invoice",
    "format": "pdf",
    "data": {...}
  }'

# Response includes URL, use it for notification
```

**Option B: Kafka (single message, fully async)**

```json
{
  "type": "generate_and_notify",
  "request_id": "invoice-workflow-001",
  "tenant_id": "1",
  "document": {
    "request_id": "inv-001",
    "tenant_id": "1",
    "document_type": "invoice",
    "format": "pdf",
    "template_id": "invoice_fiscal",
    "data": {
      "invoice": { "number": "FAC-001", "ncf": "B0100000001" },
      "totals": { "total": 1770.00 }
    },
    "store": true
  },
  "notification": {
    "request_id": "notif-001",
    "tenant_id": "1",
    "channel": "whatsapp",
    "recipient": "18296630497",
    "message": "Factura FAC-001 lista. Total: RD$ 1,770.00"
  }
}
```

---

### Use Case 2: Generate Monthly Report (Excel)

```json
{
  "type": "generate_document",
  "request_id": "report-nov-2024",
  "tenant_id": "1",
  "document_type": "report",
  "format": "excel",
  "template_id": "monthly_sales_report",
  "data": {
    "report": {
      "title": "Reporte de Ventas - Noviembre 2024",
      "period": { "start": "2024-11-01", "end": "2024-11-30" }
    },
    "rows": [
      { "date": "2024-11-01", "product": "Producto A", "quantity": 10, "amount": 1500.00 },
      { "date": "2024-11-02", "product": "Producto B", "quantity": 5, "amount": 750.00 }
    ],
    "summary": {
      "total_sales": 50000.00,
      "total_items": 500
    }
  },
  "store": true,
  "callback_url": "https://core-service.com/webhooks/reports"
}
```

---

### Use Case 3: Batch Invoice Generation (End of Month)

```json
{
  "type": "batch_generate",
  "request_id": "eom-batch-2024-11",
  "tenant_id": "1",
  "batch_id": "invoices-november-2024",
  "documents": [
    {
      "request_id": "inv-001",
      "tenant_id": "1",
      "document_type": "invoice",
      "format": "pdf",
      "template_id": "invoice_fiscal",
      "data": { "invoice": { "number": "FAC-001" }, "customer": { "name": "Cliente 1" } },
      "store": true
    },
    {
      "request_id": "inv-002",
      "tenant_id": "1",
      "document_type": "invoice",
      "format": "pdf",
      "template_id": "invoice_fiscal",
      "data": { "invoice": { "number": "FAC-002" }, "customer": { "name": "Cliente 2" } },
      "store": true
    }
  ],
  "parallel": true,
  "callback_url": "https://core-service.com/webhooks/batch-complete"
}
```

---

### Use Case 4: Send Document via Email

```json
{
  "type": "send_notification",
  "request_id": "email-001",
  "tenant_id": "1",
  "channel": "email",
  "recipient": "cliente@example.com",
  "subject": "Su factura electronica - FAC-2024-001",
  "message": "Estimado cliente,\n\nAdjunto encontrara su factura electronica.\n\nGracias por su preferencia.",
  "document_id": "doc-uuid-already-generated"
}
```

---

## Authentication

### JWT Token Structure

```json
{
  "sub": "user-100",
  "tenant_id": "1",
  "roles": ["user", "admin"],
  "exp": 1732521600
}
```

### Environment Variables

```bash
JWT_SECRET=your-secret-key
JWT_EXPIRATION_HOURS=24
JWT_ISSUER=document-service
```

---

## Error Handling

### HTTP Error Responses

```json
{
  "error": "Rate limit exceeded",
  "retry_after": 60
}
```

```json
{
  "error": "Failed to generate document",
  "details": "Template not found: invalid_template"
}
```

### Kafka DLQ Messages

Failed messages are sent to `document-dlq`:

```json
{
  "original_message": { ... },
  "error": "Template rendering failed",
  "failed_at": "2024-11-24T10:30:00Z",
  "retry_count": 3
}
```

---

## Quick Reference

### Document Types

| Type | Description |
|------|-------------|
| `invoice` | Factura fiscal |
| `quotation` | Cotizacion |
| `report` | Reporte (Excel default) |
| `receipt` | Recibo |
| `credit_note` | Nota de credito |
| `custom_*` | Tipo personalizado |

### Output Formats

| Format | Extension | MIME Type |
|--------|-----------|-----------|
| `pdf` | .pdf | application/pdf |
| `excel` | .xlsx | application/vnd.openxmlformats-officedocument.spreadsheetml.sheet |
| `csv` | .csv | text/csv |
| `html` | .html | text/html |

### Notification Channels

| Channel | Recipient Format | Notes |
|---------|------------------|-------|
| `email` | email@example.com | Supports attachments |
| `whatsapp` | 18296630497 | Dominican format auto-normalized |
| `sms` | 18296630497 | Coming soon |

### Priority Levels

| Priority | Expected Processing Time |
|----------|-------------------------|
| `urgent` | < 30 seconds |
| `high` | < 1 minute |
| `normal` | < 5 minutes |
| `low` | Best effort |

---

## Configuration

### Environment Variables (core-service)

```bash
# PDF Services HTTP
PDF_SERVICE_URL=http://localhost:8080

# PDF Services Kafka
KAFKA_BROKERS=localhost:9092
KAFKA_TOPIC_DOCUMENTS=document-generate-request
KAFKA_TOPIC_NOTIFICATIONS=notification-dispatch-request
KAFKA_TOPIC_EVENTS=document-events
```

---

## Testing with CLI

```bash
# Test WhatsApp
cargo run --bin local-test -- whatsapp send-text -p 18296630497 -m "Test message"

# Test PDF generation
cargo run --bin local-test -- generate -d invoice

# Test Kafka
cargo run --bin local-test -- kafka send -t document-generate-request -m '{"type":"generate_document",...}'

# Check health
cargo run --bin local-test -- health
```
