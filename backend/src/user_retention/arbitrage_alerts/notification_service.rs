use std::collections::HashMap;
use std::sync::Arc;
use std::str::FromStr;
use tokio::sync::{RwLock, mpsc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use reqwest::Client;
use crate::types::*;
use crate::user_retention::arbitrage_alerts::alert_manager::{Alert, NotificationChannel};
use crate::risk_management::RiskError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationRequest {
    pub id: Uuid,
    pub user_id: Uuid,
    pub channel: NotificationChannel,
    pub alert: Alert,
    pub created_at: DateTime<Utc>,
    pub retry_count: u32,
    pub max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationResponse {
    pub request_id: Uuid,
    pub success: bool,
    pub error_message: Option<String>,
    pub delivered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailNotification {
    pub to: String,
    pub subject: String,
    pub html_body: String,
    pub text_body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushNotification {
    pub device_token: String,
    pub title: String,
    pub body: String,
    pub data: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMessage {
    pub user_id: Uuid,
    pub message_type: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    pub alert_id: Uuid,
    pub user_id: Uuid,
    pub opportunity: serde_json::Value,
    pub priority: String,
    pub created_at: DateTime<Utc>,
}

pub struct NotificationService {
    notification_queue: Arc<RwLock<Vec<NotificationRequest>>>,
    websocket_connections: Arc<RwLock<HashMap<Uuid, mpsc::UnboundedSender<WebSocketMessage>>>>,
    user_emails: Arc<RwLock<HashMap<Uuid, String>>>,
    user_device_tokens: Arc<RwLock<HashMap<Uuid, Vec<String>>>>,
    http_client: Client,
    email_config: EmailConfig,
    push_config: PushConfig,
}

#[derive(Debug, Clone)]
pub struct EmailConfig {
    pub smtp_server: String,
    pub smtp_port: u16,
    pub username: String,
    pub password: String,
    pub from_address: String,
    pub from_name: String,
}

#[derive(Debug, Clone)]
pub struct PushConfig {
    pub fcm_server_key: String,
    pub apns_key_id: String,
    pub apns_team_id: String,
    pub apns_private_key: String,
}

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            smtp_server: std::env::var("SMTP_SERVER").unwrap_or_else(|_| "smtp.gmail.com".to_string()),
            smtp_port: std::env::var("SMTP_PORT")
                .unwrap_or_else(|_| "587".to_string())
                .parse()
                .unwrap_or(587),
            username: std::env::var("SMTP_USERNAME").unwrap_or_default(),
            password: std::env::var("SMTP_PASSWORD").unwrap_or_default(),
            from_address: std::env::var("FROM_EMAIL").unwrap_or_else(|_| "alerts@hyperdex.trade".to_string()),
            from_name: std::env::var("FROM_NAME").unwrap_or_else(|_| "HyperDEX Alerts".to_string()),
        }
    }
}

impl Default for PushConfig {
    fn default() -> Self {
        Self {
            fcm_server_key: std::env::var("FCM_SERVER_KEY").unwrap_or_default(),
            apns_key_id: std::env::var("APNS_KEY_ID").unwrap_or_default(),
            apns_team_id: std::env::var("APNS_TEAM_ID").unwrap_or_default(),
            apns_private_key: std::env::var("APNS_PRIVATE_KEY").unwrap_or_default(),
        }
    }
}

impl NotificationService {
    pub fn new() -> Self {
        Self {
            notification_queue: Arc::new(RwLock::new(Vec::new())),
            websocket_connections: Arc::new(RwLock::new(HashMap::new())),
            user_emails: Arc::new(RwLock::new(HashMap::new())),
            user_device_tokens: Arc::new(RwLock::new(HashMap::new())),
            http_client: Client::new(),
            email_config: EmailConfig::default(),
            push_config: PushConfig::default(),
        }
    }

    pub async fn start_processing(&self) -> Result<(), RiskError> {
        let service = Arc::new(self.clone());
        
        // Start notification processing loop
        tokio::spawn(async move {
            service.notification_processing_loop().await;
        });

        tracing::info!("Notification service started");
        Ok(())
    }

    pub async fn queue_notification(&self, request: NotificationRequest) -> Result<(), RiskError> {
        let mut queue = self.notification_queue.write().await;
        queue.push(request);
        tracing::debug!("Notification queued: {}", queue.len());
        Ok(())
    }

    pub async fn register_websocket_connection(&self, user_id: Uuid, sender: mpsc::UnboundedSender<WebSocketMessage>) {
        let mut connections = self.websocket_connections.write().await;
        connections.insert(user_id, sender);
        tracing::info!("WebSocket connection registered for user {}", user_id);
    }

    pub async fn unregister_websocket_connection(&self, user_id: Uuid) {
        let mut connections = self.websocket_connections.write().await;
        connections.remove(&user_id);
        tracing::info!("WebSocket connection unregistered for user {}", user_id);
    }

    pub async fn register_user_email(&self, user_id: Uuid, email: String) {
        let mut emails = self.user_emails.write().await;
        emails.insert(user_id, email);
    }

    pub async fn register_device_token(&self, user_id: Uuid, token: String) {
        let mut tokens = self.user_device_tokens.write().await;
        tokens.entry(user_id).or_insert_with(Vec::new).push(token);
    }

    async fn notification_processing_loop(&self) {
        loop {
            let requests = {
                let mut queue = self.notification_queue.write().await;
                let requests: Vec<NotificationRequest> = queue.drain(..).collect();
                requests
            };

            for request in requests {
                match self.process_notification(request.clone()).await {
                    Ok(_) => {
                        tracing::info!("Notification {} delivered successfully", request.id);
                    }
                    Err(e) => {
                        tracing::error!("Failed to deliver notification {}: {:?}", request.id, e);
                        
                        // Retry if under max retries
                        if request.retry_count < request.max_retries {
                            let retry_request = NotificationRequest {
                                retry_count: request.retry_count + 1,
                                ..request
                            };
                            
                            // Re-queue with delay
                            let service = Arc::new(self.clone());
                            tokio::spawn(async move {
                                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                                let _ = service.queue_notification(retry_request).await;
                            });
                        }
                    }
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    async fn process_notification(&self, request: NotificationRequest) -> Result<NotificationResponse, RiskError> {
        let result = match &request.channel {
            NotificationChannel::WebSocket => {
                self.send_websocket_notification(&request).await
            }
            NotificationChannel::Email => {
                self.send_email_notification(&request).await
            }
            NotificationChannel::PushNotification => {
                self.send_push_notification(&request).await
            }
            NotificationChannel::Webhook { url } => {
                self.send_webhook_notification(&request, url).await
            }
        };

        match result {
            Ok(_) => Ok(NotificationResponse {
                request_id: request.id,
                success: true,
                error_message: None,
                delivered_at: Utc::now(),
            }),
            Err(e) => Ok(NotificationResponse {
                request_id: request.id,
                success: false,
                error_message: Some(e.to_string()),
                delivered_at: Utc::now(),
            }),
        }
    }

    async fn send_websocket_notification(&self, request: &NotificationRequest) -> Result<(), RiskError> {
        let connections = self.websocket_connections.read().await;
        
        if let Some(sender) = connections.get(&request.user_id) {
            let message = WebSocketMessage {
                user_id: request.user_id,
                message_type: "arbitrage_alert".to_string(),
                payload: serde_json::to_value(&request.alert)
                    .map_err(|e| RiskError::SerializationError(e.to_string()))?,
            };

            sender.send(message)
                .map_err(|_| RiskError::NotificationError("WebSocket send failed".to_string()))?;
            
            Ok(())
        } else {
            Err(RiskError::NotFound("WebSocket connection not found".to_string()))
        }
    }

    async fn send_email_notification(&self, request: &NotificationRequest) -> Result<(), RiskError> {
        let emails = self.user_emails.read().await;
        
        if let Some(email) = emails.get(&request.user_id) {
            let email_notification = self.create_email_content(&request.alert).await?;
            
            // In a real implementation, you would use an email service like SendGrid, AWS SES, etc.
            // For now, we'll simulate the email sending
            tracing::info!("Sending email to {} for alert {}", email, request.alert.id);
            
            // Simulate email sending delay
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            Ok(())
        } else {
            Err(RiskError::NotFound("User email not found".to_string()))
        }
    }

    async fn send_push_notification(&self, request: &NotificationRequest) -> Result<(), RiskError> {
        let tokens = self.user_device_tokens.read().await;
        
        if let Some(device_tokens) = tokens.get(&request.user_id) {
            let push_notification = self.create_push_content(&request.alert).await?;
            
            for token in device_tokens {
                // In a real implementation, you would use FCM, APNS, etc.
                tracing::info!("Sending push notification to device {} for alert {}", token, request.alert.id);
                
                // Simulate push notification sending
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
            
            Ok(())
        } else {
            Err(RiskError::NotFound("User device tokens not found".to_string()))
        }
    }

    async fn send_webhook_notification(&self, request: &NotificationRequest, url: &str) -> Result<(), RiskError> {
        let webhook_payload = WebhookPayload {
            alert_id: request.alert.id,
            user_id: request.user_id,
            opportunity: serde_json::to_value(&request.alert.opportunity)
                .map_err(|e| RiskError::SerializationError(e.to_string()))?,
            priority: format!("{:?}", request.alert.priority),
            created_at: request.alert.created_at,
        };

        let response = self.http_client
            .post(url)
            .json(&webhook_payload)
            .timeout(tokio::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| RiskError::NotificationError(format!("Webhook request failed: {}", e)))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(RiskError::NotificationError(format!("Webhook returned status: {}", response.status())))
        }
    }

    async fn create_email_content(&self, alert: &Alert) -> Result<EmailNotification, RiskError> {
        let opportunity = &alert.opportunity;
        
        let subject = format!(
            "🚨 Arbitrage Alert: {:.2}% profit on {}/{}",
            f64::try_from(opportunity.profit_percentage).unwrap_or(0.0) * 100.0,
            opportunity.token_pair.base_token,
            opportunity.token_pair.quote_token
        );

        let html_body = format!(
            r#"
            <html>
            <body>
                <h2>🚨 Arbitrage Opportunity Detected</h2>
                <div style="background-color: #f0f8ff; padding: 20px; border-radius: 10px; margin: 20px 0;">
                    <h3>Opportunity Details</h3>
                    <p><strong>Token Pair:</strong> {}/{}</p>
                    <p><strong>Profit:</strong> {:.2}% (${:.2})</p>
                    <p><strong>Source DEX:</strong> {} (${:.2})</p>
                    <p><strong>Target DEX:</strong> {} (${:.2})</p>
                    <p><strong>Liquidity:</strong> ${:.2}</p>
                    <p><strong>Estimated Gas:</strong> ${:.2}</p>
                    <p><strong>Net Profit:</strong> ${:.2}</p>
                    <p><strong>Confidence:</strong> {:.1}%</p>
                    <p><strong>Expires:</strong> {}</p>
                </div>
                <div style="margin: 20px 0;">
                    <a href="https://hyperdex.trade/arbitrage/{}" 
                       style="background-color: #4CAF50; color: white; padding: 15px 32px; text-decoration: none; border-radius: 5px;">
                        Execute Arbitrage
                    </a>
                </div>
                <p><small>This alert was generated by HyperDEX. <a href="https://hyperdex.trade/unsubscribe">Unsubscribe</a></small></p>
            </body>
            </html>
            "#,
            opportunity.token_pair.base_token,
            opportunity.token_pair.quote_token,
            f64::try_from(opportunity.profit_percentage).unwrap_or(0.0) * 100.0,
            f64::try_from(opportunity.estimated_profit_usd).unwrap_or(0.0),
            opportunity.source_dex,
            f64::try_from(opportunity.source_price).unwrap_or(0.0),
            opportunity.target_dex,
            f64::try_from(opportunity.target_price).unwrap_or(0.0),
            f64::try_from(opportunity.liquidity_available).unwrap_or(0.0),
            f64::try_from(opportunity.estimated_gas_cost).unwrap_or(0.0),
            f64::try_from(opportunity.net_profit_usd).unwrap_or(0.0),
            opportunity.confidence_score * 100.0,
            opportunity.expires_at.format("%Y-%m-%d %H:%M:%S UTC"),
            opportunity.id
        );

        let text_body = format!(
            "Arbitrage Opportunity Detected!\n\n\
             Token Pair: {}/{}\n\
             Profit: {:.2}% (${:.2})\n\
             Source: {} (${:.2})\n\
             Target: {} (${:.2})\n\
             Liquidity: ${:.2}\n\
             Gas Cost: ${:.2}\n\
             Net Profit: ${:.2}\n\
             Confidence: {:.1}%\n\
             Expires: {}\n\n\
             Execute at: https://hyperdex.trade/arbitrage/{}",
            opportunity.token_pair.base_token,
            opportunity.token_pair.quote_token,
            f64::try_from(opportunity.profit_percentage).unwrap_or(0.0) * 100.0,
            f64::try_from(opportunity.estimated_profit_usd).unwrap_or(0.0),
            opportunity.source_dex,
            f64::try_from(opportunity.source_price).unwrap_or(0.0),
            opportunity.target_dex,
            f64::try_from(opportunity.target_price).unwrap_or(0.0),
            f64::try_from(opportunity.liquidity_available).unwrap_or(0.0),
            f64::try_from(opportunity.estimated_gas_cost).unwrap_or(0.0),
            f64::try_from(opportunity.net_profit_usd).unwrap_or(0.0),
            opportunity.confidence_score * 100.0,
            opportunity.expires_at.format("%Y-%m-%d %H:%M:%S UTC"),
            opportunity.id
        );

        Ok(EmailNotification {
            to: "user@example.com".to_string(), // This would be the actual user email
            subject,
            html_body,
            text_body,
        })
    }

    async fn create_push_content(&self, alert: &Alert) -> Result<PushNotification, RiskError> {
        let opportunity = &alert.opportunity;
        
        let title = "🚨 Arbitrage Alert";
        let profit_percentage = f64::try_from(opportunity.profit_percentage).unwrap_or(0.0);
        let body = format!(
            "{:.1}% profit on {}/{} - ${:.0} net profit",
            profit_percentage * 100.0,
            opportunity.token_pair.base_token,
            opportunity.token_pair.quote_token,
            f64::try_from(opportunity.net_profit_usd).unwrap_or(0.0)
        );

        let mut data = HashMap::new();
        data.insert("alert_id".to_string(), alert.id.to_string());
        data.insert("opportunity_id".to_string(), opportunity.id.to_string());
        data.insert("profit_percentage".to_string(), opportunity.profit_percentage.to_string());
        data.insert("net_profit".to_string(), opportunity.net_profit_usd.to_string());

        Ok(PushNotification {
            device_token: "mock_device_token".to_string(), // This would be the actual device token
            title: title.to_string(),
            body,
            data,
        })
    }
}

impl Clone for NotificationService {
    fn clone(&self) -> Self {
        Self {
            notification_queue: Arc::clone(&self.notification_queue),
            websocket_connections: Arc::clone(&self.websocket_connections),
            user_emails: Arc::clone(&self.user_emails),
            user_device_tokens: Arc::clone(&self.user_device_tokens),
            http_client: self.http_client.clone(),
            email_config: self.email_config.clone(),
            push_config: self.push_config.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user_retention::arbitrage_alerts::detector::{ArbitrageOpportunity, TokenPair};
    use crate::user_retention::arbitrage_alerts::alert_manager::{AlertPriority, AlertStatus};

    #[tokio::test]
    async fn test_notification_service_creation() {
        let service = NotificationService::new();
        let queue = service.notification_queue.read().await;
        assert_eq!(queue.len(), 0);
    }

    #[tokio::test]
    async fn test_websocket_connection_registration() {
        let service = NotificationService::new();
        let user_id = Uuid::new_v4();
        let (sender, _receiver) = mpsc::unbounded_channel();
        
        service.register_websocket_connection(user_id, sender).await;
        
        let connections = service.websocket_connections.read().await;
        assert!(connections.contains_key(&user_id));
    }

    #[tokio::test]
    async fn test_email_content_creation() {
        let service = NotificationService::new();
        
        let opportunity = ArbitrageOpportunity {
            id: Uuid::new_v4(),
            token_pair: TokenPair {
                base_token: "ETH".to_string(),
                quote_token: "USDC".to_string(),
                base_token_address: "0x123".to_string(),
                quote_token_address: "0x456".to_string(),
            },
            source_dex: "Uniswap".to_string(),
            target_dex: "Curve".to_string(),
            source_price: Decimal::from_str("3400").unwrap(),
            target_price: Decimal::from_str("3468").unwrap(),
            price_difference: Decimal::from_str("68").unwrap(),
            profit_percentage: Decimal::from_str("0.02").unwrap(),
            estimated_profit_usd: Decimal::from_str("680").unwrap(),
            estimated_gas_cost: Decimal::from_str("50").unwrap(),
            net_profit_usd: Decimal::from_str("630").unwrap(),
            liquidity_available: Decimal::from_str("50000").unwrap(),
            execution_time_estimate: 15000,
            confidence_score: 0.85,
            detected_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::minutes(5),
            chain_id: 1,
        };

        let alert = Alert {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            opportunity,
            priority: AlertPriority::High,
            created_at: Utc::now(),
            sent_at: None,
            status: AlertStatus::Pending,
            delivery_attempts: 0,
        };

        let email_content = service.create_email_content(&alert).await.unwrap();
        assert!(email_content.subject.contains("Arbitrage Alert"));
        assert!(email_content.html_body.contains("ETH/USDC"));
        assert!(email_content.text_body.contains("2.00%"));
    }
}
