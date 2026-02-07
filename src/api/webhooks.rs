use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Webhook event types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WebhookEvent {
    VMCreated,
    VMDeleted,
    VMStarted,
    VMStopped,
    VMRestarted,
    VMFailed,
    VMHealthChanged,
    SnapshotCreated,
    SnapshotRestored,
    BackupCompleted,
    BackupFailed,
    MigrationStarted,
    MigrationCompleted,
    AlertTriggered,
    AlertResolved,
    Custom(String),
}

impl std::fmt::Display for WebhookEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebhookEvent::VMCreated => write!(f, "vm.created"),
            WebhookEvent::VMDeleted => write!(f, "vm.deleted"),
            WebhookEvent::VMStarted => write!(f, "vm.started"),
            WebhookEvent::VMStopped => write!(f, "vm.stopped"),
            WebhookEvent::VMRestarted => write!(f, "vm.restarted"),
            WebhookEvent::VMFailed => write!(f, "vm.failed"),
            WebhookEvent::VMHealthChanged => write!(f, "vm.health_changed"),
            WebhookEvent::SnapshotCreated => write!(f, "snapshot.created"),
            WebhookEvent::SnapshotRestored => write!(f, "snapshot.restored"),
            WebhookEvent::BackupCompleted => write!(f, "backup.completed"),
            WebhookEvent::BackupFailed => write!(f, "backup.failed"),
            WebhookEvent::MigrationStarted => write!(f, "migration.started"),
            WebhookEvent::MigrationCompleted => write!(f, "migration.completed"),
            WebhookEvent::AlertTriggered => write!(f, "alert.triggered"),
            WebhookEvent::AlertResolved => write!(f, "alert.resolved"),
            WebhookEvent::Custom(name) => write!(f, "custom.{}", name),
        }
    }
}

/// Webhook configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    pub id: String,
    pub name: String,
    pub url: String,
    pub events: Vec<WebhookEvent>,
    pub secret: Option<String>,
    pub headers: HashMap<String, String>,
    pub enabled: bool,
    pub retry_count: u32,
    pub retry_delay_secs: u64,
    pub timeout_secs: u64,
    pub created_at: DateTime<Utc>,
    pub last_triggered: Option<DateTime<Utc>>,
    pub delivery_count: u64,
    pub failure_count: u64,
}

impl WebhookConfig {
    pub fn new(name: impl Into<String>, url: impl Into<String>) -> Self {
        let name_str = name.into();
        Self {
            id: format!("wh-{}-{}", name_str.to_lowercase().replace(' ', "-"), Utc::now().timestamp()),
            name: name_str,
            url: url.into(),
            events: Vec::new(),
            secret: None,
            headers: HashMap::new(),
            enabled: true,
            retry_count: 3,
            retry_delay_secs: 10,
            timeout_secs: 30,
            created_at: Utc::now(),
            last_triggered: None,
            delivery_count: 0,
            failure_count: 0,
        }
    }

    pub fn add_event(&mut self, event: WebhookEvent) {
        if !self.events.contains(&event) {
            self.events.push(event);
        }
    }

    pub fn remove_event(&mut self, event: &WebhookEvent) -> bool {
        let len_before = self.events.len();
        self.events.retain(|e| e != event);
        self.events.len() < len_before
    }

    pub fn with_secret(mut self, secret: impl Into<String>) -> Self {
        self.secret = Some(secret.into());
        self
    }

    pub fn add_header(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.headers.insert(key.into(), value.into());
    }

    pub fn with_retry(mut self, count: u32, delay_secs: u64) -> Self {
        self.retry_count = count;
        self.retry_delay_secs = delay_secs;
        self
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn record_delivery(&mut self, success: bool) {
        self.delivery_count += 1;
        self.last_triggered = Some(Utc::now());
        if !success {
            self.failure_count += 1;
        }
    }

    pub fn subscribes_to(&self, event: &WebhookEvent) -> bool {
        self.events.contains(event)
    }

    pub fn success_rate(&self) -> f64 {
        if self.delivery_count == 0 {
            return 100.0;
        }
        ((self.delivery_count - self.failure_count) as f64 / self.delivery_count as f64) * 100.0
    }

    pub fn event_count(&self) -> usize {
        self.events.len()
    }
}

/// Webhook delivery payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookPayload {
    pub event: String,
    pub timestamp: DateTime<Utc>,
    pub data: HashMap<String, String>,
    pub source: String,
}

impl WebhookPayload {
    pub fn new(event: &WebhookEvent) -> Self {
        Self {
            event: event.to_string(),
            timestamp: Utc::now(),
            data: HashMap::new(),
            source: "zorvia".to_string(),
        }
    }

    pub fn add_data(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.data.insert(key.into(), value.into());
    }

    pub fn data_count(&self) -> usize {
        self.data.len()
    }
}

/// Webhook manager
pub struct WebhookManager {
    webhooks: HashMap<String, WebhookConfig>,
}

impl WebhookManager {
    pub fn new() -> Self {
        Self {
            webhooks: HashMap::new(),
        }
    }

    pub fn register(&mut self, webhook: WebhookConfig) -> String {
        let id = webhook.id.clone();
        self.webhooks.insert(id.clone(), webhook);
        id
    }

    pub fn unregister(&mut self, id: &str) -> bool {
        self.webhooks.remove(id).is_some()
    }

    pub fn get(&self, id: &str) -> Option<&WebhookConfig> {
        self.webhooks.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut WebhookConfig> {
        self.webhooks.get_mut(id)
    }

    pub fn list(&self) -> Vec<&WebhookConfig> {
        self.webhooks.values().collect()
    }

    pub fn active_webhooks(&self) -> Vec<&WebhookConfig> {
        self.webhooks.values().filter(|w| w.enabled).collect()
    }

    pub fn webhooks_for_event(&self, event: &WebhookEvent) -> Vec<&WebhookConfig> {
        self.webhooks
            .values()
            .filter(|w| w.enabled && w.subscribes_to(event))
            .collect()
    }

    pub fn webhook_count(&self) -> usize {
        self.webhooks.len()
    }
}

impl Default for WebhookManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_event_display() {
        assert_eq!(WebhookEvent::VMCreated.to_string(), "vm.created");
        assert_eq!(WebhookEvent::VMDeleted.to_string(), "vm.deleted");
        assert_eq!(WebhookEvent::VMStarted.to_string(), "vm.started");
        assert_eq!(WebhookEvent::BackupCompleted.to_string(), "backup.completed");
        assert_eq!(WebhookEvent::AlertTriggered.to_string(), "alert.triggered");
        assert_eq!(WebhookEvent::Custom("deploy".to_string()).to_string(), "custom.deploy");
    }

    #[test]
    fn test_webhook_config_new() {
        let wh = WebhookConfig::new("test", "https://example.com/webhook");
        assert_eq!(wh.name, "test");
        assert_eq!(wh.url, "https://example.com/webhook");
        assert!(wh.enabled);
        assert_eq!(wh.retry_count, 3);
        assert_eq!(wh.delivery_count, 0);
    }

    #[test]
    fn test_webhook_add_event() {
        let mut wh = WebhookConfig::new("test", "https://example.com");
        wh.add_event(WebhookEvent::VMCreated);
        wh.add_event(WebhookEvent::VMDeleted);
        wh.add_event(WebhookEvent::VMCreated); // Duplicate

        assert_eq!(wh.event_count(), 2);
        assert!(wh.subscribes_to(&WebhookEvent::VMCreated));
    }

    #[test]
    fn test_webhook_remove_event() {
        let mut wh = WebhookConfig::new("test", "https://example.com");
        wh.add_event(WebhookEvent::VMCreated);

        assert!(wh.remove_event(&WebhookEvent::VMCreated));
        assert!(!wh.remove_event(&WebhookEvent::VMCreated));
        assert_eq!(wh.event_count(), 0);
    }

    #[test]
    fn test_webhook_with_secret() {
        let wh = WebhookConfig::new("test", "https://example.com")
            .with_secret("my-secret");
        assert_eq!(wh.secret, Some("my-secret".to_string()));
    }

    #[test]
    fn test_webhook_add_header() {
        let mut wh = WebhookConfig::new("test", "https://example.com");
        wh.add_header("X-Custom", "value");
        assert_eq!(wh.headers.get("X-Custom"), Some(&"value".to_string()));
    }

    #[test]
    fn test_webhook_with_retry() {
        let wh = WebhookConfig::new("test", "https://example.com")
            .with_retry(5, 30);
        assert_eq!(wh.retry_count, 5);
        assert_eq!(wh.retry_delay_secs, 30);
    }

    #[test]
    fn test_webhook_enable_disable() {
        let mut wh = WebhookConfig::new("test", "https://example.com");
        assert!(wh.enabled);

        wh.disable();
        assert!(!wh.enabled);

        wh.enable();
        assert!(wh.enabled);
    }

    #[test]
    fn test_webhook_record_delivery() {
        let mut wh = WebhookConfig::new("test", "https://example.com");

        wh.record_delivery(true);
        wh.record_delivery(true);
        wh.record_delivery(false);

        assert_eq!(wh.delivery_count, 3);
        assert_eq!(wh.failure_count, 1);
        assert!(wh.last_triggered.is_some());
    }

    #[test]
    fn test_webhook_success_rate() {
        let mut wh = WebhookConfig::new("test", "https://example.com");
        assert_eq!(wh.success_rate(), 100.0);

        wh.record_delivery(true);
        wh.record_delivery(false);

        assert_eq!(wh.success_rate(), 50.0);
    }

    #[test]
    fn test_webhook_payload_new() {
        let mut payload = WebhookPayload::new(&WebhookEvent::VMCreated);
        payload.add_data("vm_name", "test-vm");
        payload.add_data("namespace", "default");

        assert_eq!(payload.event, "vm.created");
        assert_eq!(payload.data_count(), 2);
        assert_eq!(payload.source, "zorvia");
    }

    #[test]
    fn test_webhook_manager_register() {
        let mut manager = WebhookManager::new();
        let wh = WebhookConfig::new("test", "https://example.com");
        let id = manager.register(wh);

        assert_eq!(manager.webhook_count(), 1);
        assert!(manager.get(&id).is_some());
    }

    #[test]
    fn test_webhook_manager_unregister() {
        let mut manager = WebhookManager::new();
        let id = manager.register(WebhookConfig::new("test", "https://example.com"));

        assert!(manager.unregister(&id));
        assert!(!manager.unregister(&id));
    }

    #[test]
    fn test_webhook_manager_active_webhooks() {
        let mut manager = WebhookManager::new();

        let mut disabled = WebhookConfig::new("disabled", "https://example.com");
        disabled.disable();

        manager.register(WebhookConfig::new("active", "https://example.com"));
        manager.register(disabled);

        assert_eq!(manager.active_webhooks().len(), 1);
    }

    #[test]
    fn test_webhook_manager_webhooks_for_event() {
        let mut manager = WebhookManager::new();

        let mut wh1 = WebhookConfig::new("vm-events", "https://example.com/vm");
        wh1.add_event(WebhookEvent::VMCreated);
        wh1.add_event(WebhookEvent::VMDeleted);

        let mut wh2 = WebhookConfig::new("backup-events", "https://example.com/backup");
        wh2.add_event(WebhookEvent::BackupCompleted);

        manager.register(wh1);
        manager.register(wh2);

        let vm_hooks = manager.webhooks_for_event(&WebhookEvent::VMCreated);
        assert_eq!(vm_hooks.len(), 1);

        let backup_hooks = manager.webhooks_for_event(&WebhookEvent::BackupCompleted);
        assert_eq!(backup_hooks.len(), 1);

        let no_hooks = manager.webhooks_for_event(&WebhookEvent::AlertTriggered);
        assert_eq!(no_hooks.len(), 0);
    }
}
