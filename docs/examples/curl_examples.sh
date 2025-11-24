#!/bin/bash
# PDF Services - cURL Examples
# Quick reference for testing the API

BASE_URL="http://localhost:8080"
TENANT_ID="1"
USER_ID="100"

# ============================================
# Health Checks
# ============================================

echo "=== Health Check ==="
curl -s "$BASE_URL/health" | jq

echo -e "\n=== Readiness Check ==="
curl -s "$BASE_URL/ready" | jq

# ============================================
# Generate Invoice (Sync) - PDF
# ============================================

echo -e "\n=== Generate Invoice (Sync) ==="
curl -X POST "$BASE_URL/api/v1/documents/generate/sync" \
  -H "Content-Type: application/json" \
  -H "X-Tenant-Id: $TENANT_ID" \
  -H "X-User-Id: $USER_ID" \
  -d '{
    "template_id": "invoice_fiscal",
    "document_type": "invoice",
    "format": "pdf",
    "priority": "high",
    "metadata": {
      "tenant_id": 1,
      "user_id": 100
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
        },
        {
          "code": "PROD-002",
          "description": "Otro producto",
          "quantity": 5,
          "unit_price": 200.00,
          "discount": 10,
          "tax_rate": 18
        }
      ],
      "totals": {
        "subtotal": 2400.00,
        "discount": 100.00,
        "tax": 414.00,
        "total": 2714.00
      },
      "options": {
        "include_qr": true,
        "locale": "es-DO"
      }
    }
  }' | jq

# ============================================
# Generate Report (Excel)
# ============================================

echo -e "\n=== Generate Report (Excel) ==="
curl -X POST "$BASE_URL/api/v1/documents/generate/sync" \
  -H "Content-Type: application/json" \
  -H "X-Tenant-Id: $TENANT_ID" \
  -H "X-User-Id: $USER_ID" \
  -d '{
    "template_id": "monthly_sales_report",
    "document_type": "report",
    "format": "excel",
    "priority": "normal",
    "metadata": {
      "tenant_id": 1,
      "user_id": 100
    },
    "data": {
      "report": {
        "title": "Reporte de Ventas - Noviembre 2024",
        "period": {
          "start": "2024-11-01",
          "end": "2024-11-30"
        }
      },
      "columns": ["Fecha", "Producto", "Cantidad", "Monto"],
      "rows": [
        {"date": "2024-11-01", "product": "Producto A", "quantity": 10, "amount": 1500.00},
        {"date": "2024-11-02", "product": "Producto B", "quantity": 5, "amount": 750.00},
        {"date": "2024-11-03", "product": "Producto C", "quantity": 8, "amount": 1200.00}
      ],
      "summary": {
        "total_sales": 3450.00,
        "total_items": 23
      }
    }
  }' | jq

# ============================================
# Generate Document (Async)
# ============================================

echo -e "\n=== Generate Large Report (Async) ==="
ASYNC_RESPONSE=$(curl -s -X POST "$BASE_URL/api/v1/documents/generate/async" \
  -H "Content-Type: application/json" \
  -H "X-Tenant-Id: $TENANT_ID" \
  -H "X-User-Id: $USER_ID" \
  -d '{
    "template_id": "large_report",
    "document_type": "report",
    "format": "excel",
    "priority": "low",
    "metadata": {
      "tenant_id": 1,
      "user_id": 100
    },
    "data": {
      "report": {"title": "Large Report"},
      "rows": []
    }
  }')

echo "$ASYNC_RESPONSE" | jq

# Extract document ID for status check
DOC_ID=$(echo "$ASYNC_RESPONSE" | jq -r '.id')

# ============================================
# Check Document Status
# ============================================

echo -e "\n=== Check Status ==="
if [ "$DOC_ID" != "null" ]; then
  curl -s "$BASE_URL/api/v1/documents/$DOC_ID/status" \
    -H "X-Tenant-Id: $TENANT_ID" \
    -H "X-User-Id: $USER_ID" | jq
fi

# ============================================
# List Templates
# ============================================

echo -e "\n=== List Templates ==="
curl -s "$BASE_URL/api/v1/templates" \
  -H "X-Tenant-Id: $TENANT_ID" \
  -H "X-User-Id: $USER_ID" | jq

# ============================================
# Metrics (Prometheus format)
# ============================================

echo -e "\n=== Metrics ==="
curl -s "$BASE_URL/metrics" | head -20

# ============================================
# Kafka Message Examples (for reference)
# ============================================

echo -e "\n=== Kafka Message Examples (for kcat/kafkacat) ==="

cat << 'EOF'
# Generate Document
echo '{"type":"generate_document","request_id":"req-001","tenant_id":"1","document_type":"invoice","format":"pdf","template_id":"invoice_fiscal","data":{"invoice":{"number":"FAC-001"}},"store":true}' | kcat -P -b localhost:9092 -t document-generate-request

# Send WhatsApp Notification
echo '{"type":"send_notification","request_id":"notif-001","tenant_id":"1","channel":"whatsapp","recipient":"18296630497","message":"Test message from PDF Services"}' | kcat -P -b localhost:9092 -t notification-dispatch-request

# Generate and Notify
echo '{"type":"generate_and_notify","request_id":"workflow-001","tenant_id":"1","document":{"request_id":"doc-001","tenant_id":"1","document_type":"invoice","format":"pdf","template_id":"invoice_fiscal","data":{},"store":true},"notification":{"request_id":"notif-001","tenant_id":"1","channel":"whatsapp","recipient":"18296630497","message":"Your invoice is ready"}}' | kcat -P -b localhost:9092 -t document-generate-request

# Listen to events
kcat -C -b localhost:9092 -t document-events -o end
EOF
