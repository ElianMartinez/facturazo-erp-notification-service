# 📱 EvolutionAPI Integration Documentation

## 🔧 Ambiente de Pruebas

### Base Configuration
- **API URL**: `http://5.161.120.166:8080`
- **API Version**: v2
- **Instance Name**: `FACTURAZO-ERP-DEV`
- **API Key**: `mySuperSecretKey123` (testing only)

### Authentication
All requests must include the following header:
```
apikey: mySuperSecretKey123
```

---

## 📨 API Endpoints

### 1. Send Text Message

**Endpoint**: `POST /message/sendText/{instance}`

**Example URL**: `http://5.161.120.166:8080/message/sendText/FACTURAZO-ERP-DEV`

**Request Body**:
```json
{
    "number": "18296630497",
    "text": "Su factura E310000000001 está lista para descargar",
    "delay": 0,
    "linkPreview": true,
    "mentioned": [
        "18296630497"
    ]
}
```

**Optional Fields**:
- `mentionsEveryOne`: boolean - Mention all participants in group
- `quoted`: object - Reply to a specific message
  ```json
  {
      "key": {
          "id": "<message_id>"
      },
      "message": {
          "conversation": "<original_text>"
      }
  }
  ```

### 2. Send Media (PDF Documents)

**Endpoint**: `POST /message/sendMedia/{instance}`

**Example URL**: `http://5.161.120.166:8080/message/sendMedia/FACTURAZO-ERP-DEV`

**Request Body for PDF**:
```json
{
    "number": "18296630497",
    "mediatype": "document",
    "mimetype": "application/pdf",
    "caption": "Factura Crédito Fiscal E310000000001",
    "media": "base64_encoded_pdf_content",
    "fileName": "E310000000001.pdf",
    "delay": 0,
    "linkPreview": false,
    "mentionsEveryOne": false
}
```

**Media Types Supported**:
- `document`: PDF, DOC, XLSX, etc.
- `image`: JPG, PNG, GIF
- `video`: MP4, AVI
- `audio`: MP3, OGG, AAC

---

## 🔐 Security Considerations

### Production Environment
- [ ] Use environment variables for API key
- [ ] Implement IP whitelist
- [ ] Use HTTPS in production
- [ ] Rotate API keys regularly
- [ ] Implement rate limiting

### Development Environment
- Current setup is for TESTING ONLY
- Do not use production data with test instance
- Monitor webhook responses for errors

---

## 📋 Implementation Checklist

### Phase 1: Basic Integration
- [x] Document API endpoints
- [ ] Create WhatsApp client service
- [ ] Implement send text message
- [ ] Implement send document (PDF)
- [ ] Add error handling

### Phase 2: Advanced Features
- [ ] Implement webhook handler
- [ ] Add message status tracking
- [ ] Implement retry logic
- [ ] Add queue management
- [ ] Create templates system

### Phase 3: Production Ready
- [ ] Add monitoring and logging
- [ ] Implement circuit breaker
- [ ] Add metrics collection
- [ ] Create admin dashboard
- [ ] Setup multiple instances

---

## 💻 Code Examples

### Rust Implementation

```rust
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct SendTextRequest {
    pub number: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_preview: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct SendMediaRequest {
    pub number: String,
    pub mediatype: String,
    pub mimetype: String,
    pub caption: String,
    pub media: String,  // base64
    pub file_name: String,
}

pub struct EvolutionAPIClient {
    base_url: String,
    api_key: String,
    instance: String,
    client: Client,
}

impl EvolutionAPIClient {
    pub async fn send_text(&self, request: SendTextRequest) -> Result<()> {
        let url = format!("{}/message/sendText/{}", self.base_url, self.instance);

        let response = self.client
            .post(&url)
            .header("apikey", &self.api_key)
            .json(&request)
            .send()
            .await?;

        // Handle response
        Ok(())
    }

    pub async fn send_pdf(&self, request: SendMediaRequest) -> Result<()> {
        let url = format!("{}/message/sendMedia/{}", self.base_url, self.instance);

        let response = self.client
            .post(&url)
            .header("apikey", &self.api_key)
            .json(&request)
            .send()
            .await?;

        // Handle response
        Ok(())
    }
}
```

---

## 🧪 Testing

### Manual Testing with cURL

**Send Text**:
```bash
curl -X POST http://5.161.120.166:8080/message/sendText/FACTURAZO-ERP-DEV \
  -H "Content-Type: application/json" \
  -H "apikey: mySuperSecretKey123" \
  -d '{
    "number": "18296630497",
    "text": "Test message from API"
  }'
```

**Send PDF**:
```bash
curl -X POST http://5.161.120.166:8080/message/sendMedia/FACTURAZO-ERP-DEV \
  -H "Content-Type: application/json" \
  -H "apikey: mySuperSecretKey123" \
  -d '{
    "number": "18296630497",
    "mediatype": "document",
    "mimetype": "application/pdf",
    "caption": "Test PDF",
    "media": "'$(base64 -i test.pdf)'",
    "fileName": "test.pdf"
  }'
```

---

## 📞 Dominican Republic Phone Format

### Valid Formats
- International: `+1809XXXXXXX` or `+1829XXXXXXX` or `+1849XXXXXXX`
- Without plus: `1809XXXXXXX`, `1829XXXXXXX`, `1849XXXXXXX`
- Local: `809XXXXXXX`, `829XXXXXXX`, `849XXXXXXX`

### Validation Regex
```regex
^(\+?1)?(809|829|849)\d{7}$
```

### Normalization Function
```rust
fn normalize_dominican_phone(phone: &str) -> String {
    let digits_only: String = phone.chars()
        .filter(|c| c.is_digit(10))
        .collect();

    if digits_only.starts_with("1") && digits_only.len() == 11 {
        digits_only
    } else if digits_only.len() == 10 {
        format!("1{}", digits_only)
    } else {
        digits_only
    }
}
```

---

## 🔗 Useful Links

- [EvolutionAPI Documentation](https://doc.evolution-api.com/)
- [EvolutionAPI GitHub](https://github.com/EvolutionAPI/evolution-api)
- [WhatsApp Business API](https://developers.facebook.com/docs/whatsapp)

---

## 📝 Notes

- Instance must be connected to WhatsApp before sending messages
- QR code connection required for first setup
- Session persists across restarts
- Maximum message size: 64KB for text, 100MB for media
- Rate limits apply (varies by WhatsApp account tier)

---

*Last Updated: 2024-11-24*
*Environment: Testing/Development*