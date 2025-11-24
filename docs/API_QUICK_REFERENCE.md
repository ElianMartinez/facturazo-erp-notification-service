# PDF Services - API Quick Reference

## HTTP Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| `GET` | `/health` | Health check |
| `GET` | `/ready` | Readiness check |
| `GET` | `/metrics` | Prometheus metrics |
| `POST` | `/api/v1/documents/generate/sync` | Generate document (sync) |
| `POST` | `/api/v1/documents/generate/async` | Generate document (async) |
| `POST` | `/api/v1/documents/upload` | Upload large data |
| `GET` | `/api/v1/documents/{id}/status` | Check status |
| `GET` | `/api/v1/documents/{id}/download` | Download document |
| `GET` | `/api/v1/templates` | List templates |
| `POST` | `/api/v1/templates/generate` | Generate from template |

## Required Headers

```http
Content-Type: application/json
X-Tenant-Id: 1
X-User-Id: 100
Authorization: Bearer <jwt_token>  # Optional
```

## Kafka Topics

| Topic | Direction | Purpose |
|-------|-----------|---------|
| `document-generate-request` | Produce | Document generation |
| `document-batch-request` | Produce | Batch processing |
| `notification-dispatch-request` | Produce | Send notifications |
| `document-events` | Consume | Receive responses |
| `document-dlq` | Consume | Failed messages |

## Message Types

### 1. Generate Document
```json
{"type": "generate_document", "request_id": "...", "tenant_id": "1", "document_type": "invoice", "format": "pdf", "template_id": "invoice_fiscal", "data": {...}, "store": true}
```

### 2. Send Notification
```json
{"type": "send_notification", "request_id": "...", "tenant_id": "1", "channel": "whatsapp", "recipient": "18296630497", "message": "..."}
```

### 3. Generate + Notify
```json
{"type": "generate_and_notify", "request_id": "...", "tenant_id": "1", "document": {...}, "notification": {...}}
```

### 4. Batch Generate
```json
{"type": "batch_generate", "request_id": "...", "tenant_id": "1", "batch_id": "...", "documents": [...], "parallel": true}
```

### 5. Batch Notify (Multiple Recipients)
```json
{"type": "batch_notify", "request_id": "...", "tenant_id": "1", "batch_id": "...", "channel": "whatsapp", "recipients": ["18296630497", "18095551234"], "message": "...", "parallel": true, "concurrency": 10}
```

### 6. Generate + Notify Many
```json
{"type": "generate_and_notify_many", "request_id": "...", "tenant_id": "1", "document": {...}, "notification": {"channel": "whatsapp", "recipients": ["18296630497", "18095551234"], "message": "...", "parallel": true}}
```

## Document Types

- `invoice` - Factura fiscal
- `quotation` - Cotizacion
- `report` - Reporte
- `receipt` - Recibo
- `credit_note` - Nota de credito
- `custom_*` - Personalizado

## Output Formats

| Format | Extension | Default For |
|--------|-----------|-------------|
| `pdf` | .pdf | invoice, quotation, receipt |
| `excel` | .xlsx | report |
| `csv` | .csv | data export |
| `html` | .html | preview |

## Notification Channels

| Channel | Recipient Format |
|---------|------------------|
| `email` | email@example.com |
| `whatsapp` | 18296630497 |
| `sms` | 18296630497 |

## Priority Levels

| Priority | SLA |
|----------|-----|
| `urgent` | < 30s |
| `high` | < 1min |
| `normal` | < 5min |
| `low` | Best effort |

## Status Values

- `queued` - En cola
- `processing` - Procesando
- `completed` - Completado
- `failed` - Fallido
- `cancelled` - Cancelado

## Response Events

### Document Generated
```json
{"type": "document_generated", "request_id": "...", "document_id": "...", "storage_url": "...", "file_size": 12345, "generation_time_ms": 234}
```

### Notification Sent
```json
{"type": "notification_sent", "request_id": "...", "notification_id": "...", "status": "Sent"}
```

### Batch Completed
```json
{"type": "batch_completed", "request_id": "...", "batch_id": "...", "total": 100, "successful": 98, "failed": 2}
```

## Invoice Data Structure

```json
{
  "seller": {
    "name": "Mi Empresa SRL",
    "rnc": "123456789",
    "address": "...",
    "phone": "809-555-1234",
    "email": "ventas@empresa.com"
  },
  "customer": {
    "name": "Cliente SA",
    "rnc": "987654321",
    "address": "..."
  },
  "invoice": {
    "number": "FAC-2024-001",
    "ncf": "B0100000001",
    "date": "2024-11-24",
    "due_date": "2024-12-24",
    "currency": "DOP"
  },
  "items": [
    {
      "code": "PROD-001",
      "description": "Producto",
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
```

## Environment Variables

```bash
# HTTP
PDF_SERVICE_URL=http://localhost:8080

# Kafka
KAFKA_BROKERS=localhost:9092

# WhatsApp (EvolutionAPI)
PDF_NOTIFICATION_WHATSAPP_API_URL=http://5.161.120.166:8080
PDF_NOTIFICATION_WHATSAPP_API_KEY=mySuperSecretKey123
PDF_NOTIFICATION_WHATSAPP_INSTANCE_NAME=FACTURAZO-ERP-DEV

# Email (SMTP)
PDF_NOTIFICATION_EMAIL_SMTP_HOST=smtp.gmail.com
PDF_NOTIFICATION_EMAIL_SMTP_PORT=587
PDF_NOTIFICATION_EMAIL_SMTP_USERNAME=your-email@gmail.com
PDF_NOTIFICATION_EMAIL_SMTP_PASSWORD=your-app-password
```

## CLI Testing

```bash
# Health check
cargo run --bin local-test -- health

# Send WhatsApp
cargo run --bin local-test -- whatsapp send-text -p 18296630497 -m "Test"

# Send PDF via WhatsApp
cargo run --bin local-test -- whatsapp send-pdf -p 18296630497 -c "Invoice"

# Generate test PDF
cargo run --bin local-test -- generate -d invoice

# Kafka operations
cargo run --bin local-test -- kafka check
cargo run --bin local-test -- kafka list-topics
cargo run --bin local-test -- kafka create-topics
```
