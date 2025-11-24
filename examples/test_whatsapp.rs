//! Example: Test WhatsApp integration with EvolutionAPI
//!
//! Run with: cargo run --example test_whatsapp

use pdf_services::infrastructure::notifications::EvolutionAPIClient;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration from environment
    dotenv::dotenv().ok();

    let base_url = std::env::var("PDF_NOTIFICATION_WHATSAPP_API_URL")
        .unwrap_or_else(|_| "http://5.161.120.166:8080".to_string());

    let api_key = std::env::var("PDF_NOTIFICATION_WHATSAPP_API_KEY")
        .unwrap_or_else(|_| "mySuperSecretKey123".to_string());

    let instance = std::env::var("PDF_NOTIFICATION_WHATSAPP_INSTANCE_NAME")
        .unwrap_or_else(|_| "FACTURAZO-ERP-DEV".to_string());

    println!("🚀 Testing WhatsApp Integration with EvolutionAPI");
    println!("================================================");
    println!("URL: {}", base_url);
    println!("Instance: {}", instance);
    println!();

    // Create WhatsApp client
    let client = EvolutionAPIClient::new(base_url, api_key, instance);

    // Test 1: Check connection
    println!("📱 Checking WhatsApp connection...");
    match client.is_connected().await {
        Ok(connected) => {
            if connected {
                println!("✅ WhatsApp is connected!");
            } else {
                println!("❌ WhatsApp is not connected. Please scan QR code.");
            }
        }
        Err(e) => {
            println!("⚠️ Could not check connection: {}", e);
        }
    }

    // Test 2: Send test message
    println!("\n📨 Sending test message...");

    let test_message = pdf_services::infrastructure::notifications::evolution_api::SendTextRequest {
        number: "18296630497".to_string(), // Replace with test number
        text: format!(
            "🧪 *TEST MESSAGE*\n\n\
            This is a test from the PDF Services system.\n\
            Time: {}\n\n\
            _This is an automated test message._",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
        ),
        delay: None,
        link_preview: Some(false),
        mentioned: None,
        mentions_every_one: None,
        quoted: None,
    };

    match client.send_text(test_message).await {
        Ok(message_id) => {
            println!("✅ Message sent successfully!");
            println!("   Message ID: {}", message_id);
        }
        Err(e) => {
            println!("❌ Failed to send message: {}", e);
        }
    }

    // Test 3: Send invoice notification (mock)
    println!("\n📄 Sending mock invoice notification...");

    // Create a simple PDF content (mock)
    let mock_pdf = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj\ntrailer\n<< /Root 1 0 R >>\n%%EOF";

    match client.send_invoice_notification(
        "18296630497".to_string(), // Replace with test number
        "INV-2024-0001".to_string(),
        "E310000000001".to_string(),
        "1,500.00".to_string(),
        mock_pdf.to_vec(),
    ).await {
        Ok(result) => {
            println!("✅ Invoice notification sent successfully!");
            println!("   Result: {}", result);
        }
        Err(e) => {
            println!("❌ Failed to send invoice: {}", e);
        }
    }

    println!("\n🏁 Test completed!");

    Ok(())
}