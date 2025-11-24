/**
 * PDF Services Client - TypeScript/Node.js Example
 *
 * Install dependencies:
 *   npm install axios kafkajs
 */

import axios, { AxiosInstance } from 'axios';
import { Kafka, Producer, Consumer, EachMessagePayload } from 'kafkajs';

// ============================================
// HTTP Client for Sync Operations
// ============================================

interface DocumentRequest {
  id?: string;
  template_id: string;
  document_type: 'invoice' | 'report' | 'quotation' | 'receipt';
  format: 'pdf' | 'excel' | 'csv';
  priority: 'low' | 'normal' | 'high' | 'urgent';
  data: Record<string, any>;
  metadata?: {
    tenant_id?: number;
    user_id?: number;
    tags?: Record<string, string>;
  };
}

interface DocumentResponse {
  id: string;
  status: 'queued' | 'processing' | 'completed' | 'failed';
  url?: string;
  error?: string;
  processing_time_ms: number;
}

class PdfServicesClient {
  private client: AxiosInstance;

  constructor(
    private baseUrl: string = 'http://localhost:8080',
    private tenantId: number = 1,
    private userId: number = 1,
    private jwtToken?: string
  ) {
    this.client = axios.create({
      baseURL: this.baseUrl,
      timeout: 30000,
      headers: {
        'Content-Type': 'application/json',
        'X-Tenant-Id': this.tenantId.toString(),
        'X-User-Id': this.userId.toString(),
        ...(this.jwtToken && { Authorization: `Bearer ${this.jwtToken}` }),
      },
    });
  }

  /**
   * Generate document synchronously (small documents only)
   */
  async generateSync(request: DocumentRequest): Promise<DocumentResponse> {
    const response = await this.client.post('/api/v1/documents/generate/sync', request);
    return response.data;
  }

  /**
   * Generate document asynchronously
   */
  async generateAsync(request: DocumentRequest): Promise<{ id: string; status_url: string }> {
    const response = await this.client.post('/api/v1/documents/generate/async', request);
    return response.data;
  }

  /**
   * Check document status
   */
  async getStatus(documentId: string): Promise<DocumentResponse> {
    const response = await this.client.get(`/api/v1/documents/${documentId}/status`);
    return response.data;
  }

  /**
   * Get download URL
   */
  async getDownloadUrl(documentId: string): Promise<string> {
    const response = await this.client.get(`/api/v1/documents/${documentId}/download`, {
      maxRedirects: 0,
      validateStatus: (status) => status === 302,
    });
    return response.headers.location;
  }

  /**
   * Health check
   */
  async healthCheck(): Promise<boolean> {
    try {
      const response = await this.client.get('/health');
      return response.data.status === 'healthy';
    } catch {
      return false;
    }
  }
}

// ============================================
// Kafka Client for Async Operations
// ============================================

interface KafkaDocumentRequest {
  type: 'generate_document';
  request_id: string;
  tenant_id: string;
  user_id?: string;
  document_type: string;
  format: string;
  template_id: string;
  data: Record<string, any>;
  store: boolean;
  callback_url?: string;
}

interface KafkaNotificationRequest {
  type: 'send_notification';
  request_id: string;
  tenant_id: string;
  channel: 'email' | 'whatsapp' | 'sms';
  recipient: string;
  subject?: string;
  message: string;
  document_id?: string;
  callback_url?: string;
}

interface KafkaGenerateAndNotifyRequest {
  type: 'generate_and_notify';
  request_id: string;
  tenant_id: string;
  document: Omit<KafkaDocumentRequest, 'type'>;
  notification: Omit<KafkaNotificationRequest, 'type'>;
}

class PdfServicesKafkaClient {
  private kafka: Kafka;
  private producer: Producer;
  private consumer: Consumer;

  constructor(
    private brokers: string[] = ['localhost:9092'],
    private groupId: string = 'core-service'
  ) {
    this.kafka = new Kafka({
      clientId: 'core-service',
      brokers: this.brokers,
    });
    this.producer = this.kafka.producer();
    this.consumer = this.kafka.consumer({ groupId: this.groupId });
  }

  async connect(): Promise<void> {
    await this.producer.connect();
    await this.consumer.connect();
  }

  async disconnect(): Promise<void> {
    await this.producer.disconnect();
    await this.consumer.disconnect();
  }

  /**
   * Send document generation request
   */
  async generateDocument(request: KafkaDocumentRequest): Promise<void> {
    await this.producer.send({
      topic: 'document-generate-request',
      messages: [
        {
          key: request.request_id,
          value: JSON.stringify(request),
        },
      ],
    });
  }

  /**
   * Send notification request
   */
  async sendNotification(request: KafkaNotificationRequest): Promise<void> {
    await this.producer.send({
      topic: 'notification-dispatch-request',
      messages: [
        {
          key: request.request_id,
          value: JSON.stringify(request),
        },
      ],
    });
  }

  /**
   * Generate document and send notification in one workflow
   */
  async generateAndNotify(request: KafkaGenerateAndNotifyRequest): Promise<void> {
    await this.producer.send({
      topic: 'document-generate-request',
      messages: [
        {
          key: request.request_id,
          value: JSON.stringify(request),
        },
      ],
    });
  }

  /**
   * Subscribe to document events
   */
  async subscribeToEvents(
    onMessage: (event: any) => void
  ): Promise<void> {
    await this.consumer.subscribe({ topic: 'document-events', fromBeginning: false });

    await this.consumer.run({
      eachMessage: async ({ message }: EachMessagePayload) => {
        if (message.value) {
          const event = JSON.parse(message.value.toString());
          onMessage(event);
        }
      },
    });
  }
}

// ============================================
// Usage Examples
// ============================================

async function exampleHttpUsage() {
  const client = new PdfServicesClient('http://localhost:8080', 1, 100);

  // Check health
  const isHealthy = await client.healthCheck();
  console.log('Service healthy:', isHealthy);

  // Generate invoice PDF
  const invoiceResponse = await client.generateSync({
    template_id: 'invoice_fiscal',
    document_type: 'invoice',
    format: 'pdf',
    priority: 'high',
    data: {
      seller: {
        name: 'Mi Empresa SRL',
        rnc: '123456789',
        address: 'Santo Domingo',
      },
      customer: {
        name: 'Cliente Final',
        rnc: '987654321',
      },
      invoice: {
        number: 'FAC-2024-001',
        ncf: 'B0100000001',
        date: '2024-11-24',
      },
      items: [
        {
          description: 'Producto de ejemplo',
          quantity: 10,
          unit_price: 150.0,
          tax_rate: 18,
        },
      ],
      totals: {
        subtotal: 1500.0,
        tax: 270.0,
        total: 1770.0,
      },
    },
  });

  console.log('Invoice generated:', invoiceResponse);

  if (invoiceResponse.url) {
    console.log('Download URL:', invoiceResponse.url);
  }
}

async function exampleKafkaUsage() {
  const client = new PdfServicesKafkaClient(['localhost:9092'], 'core-service');
  await client.connect();

  // Subscribe to events first
  client.subscribeToEvents((event) => {
    console.log('Received event:', event);

    switch (event.type) {
      case 'document_generated':
        console.log(`Document ${event.document_id} generated at ${event.storage_url}`);
        break;
      case 'notification_sent':
        console.log(`Notification ${event.notification_id} sent with status ${event.status}`);
        break;
      case 'batch_completed':
        console.log(`Batch ${event.batch_id}: ${event.successful}/${event.total} successful`);
        break;
    }
  });

  // Generate invoice and send via WhatsApp
  await client.generateAndNotify({
    type: 'generate_and_notify',
    request_id: `workflow-${Date.now()}`,
    tenant_id: '1',
    document: {
      request_id: `doc-${Date.now()}`,
      tenant_id: '1',
      document_type: 'invoice',
      format: 'pdf',
      template_id: 'invoice_fiscal',
      data: {
        seller: { name: 'Mi Empresa' },
        customer: { name: 'Cliente', phone: '18296630497' },
        invoice: { number: 'FAC-001', ncf: 'B0100000001' },
        totals: { total: 1770.0 },
      },
      store: true,
    },
    notification: {
      request_id: `notif-${Date.now()}`,
      tenant_id: '1',
      channel: 'whatsapp',
      recipient: '18296630497',
      message: 'Su factura FAC-001 esta lista. Total: RD$ 1,770.00',
    },
  });

  console.log('Workflow submitted');

  // Keep listening for 30 seconds
  await new Promise((resolve) => setTimeout(resolve, 30000));
  await client.disconnect();
}

// Export for use
export { PdfServicesClient, PdfServicesKafkaClient };
export type {
  DocumentRequest,
  DocumentResponse,
  KafkaDocumentRequest,
  KafkaNotificationRequest,
  KafkaGenerateAndNotifyRequest,
};
