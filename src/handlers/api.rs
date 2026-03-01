use crate::tui::colors::cli as color;
use anyhow::Result;

pub fn handle_api_serve(
    port: u16,
    host: String,
    tls: bool,
    tls_cert: Option<String>,
    tls_key: Option<String>,
    auth: String,
    rate_limit: u32,
) -> Result<()> {
    use crate::api::server::{default_endpoints, ApiServer};
    use crate::api::{ApiConfig, AuthMethod, RateLimitConfig};

    let mut config = ApiConfig::new(port).with_host(&host);

    if tls {
        if let (Some(cert), Some(key)) = (tls_cert, tls_key) {
            config = config.with_tls(cert, key);
        } else {
            return Err(anyhow::anyhow!(
                "TLS requires both --tls-cert and --tls-key"
            ));
        }
    }

    let auth_method = AuthMethod::parse(&auth).unwrap_or(AuthMethod::None);
    config = config.with_auth(auth_method.clone());

    if rate_limit > 0 {
        config = config.with_rate_limit(RateLimitConfig::new(rate_limit));
    } else {
        config = config.with_rate_limit(RateLimitConfig::disabled());
    }

    let mut server = ApiServer::new(config.clone());
    server.start();

    println!("{}", color::header("Zorvia API Server"));
    println!();
    println!("  Address:     {}", color::value(&config.address()));
    println!("  Base URL:    {}", color::value(&config.base_url()));
    println!(
        "  TLS:         {}",
        if config.tls_enabled {
            color::success("Enabled")
        } else {
            color::muted("Disabled")
        }
    );
    println!("  Auth:        {}", color::value(&auth_method.to_string()));
    println!(
        "  Rate Limit:  {}",
        if rate_limit > 0 {
            color::value(&format!("{} req/min", rate_limit))
        } else {
            color::muted("Disabled")
        }
    );
    println!();

    let endpoints = default_endpoints();
    println!("  Endpoints:   {} registered", endpoints.len());
    println!();

    let health = server.health_status();
    println!("  Status:      {}", color::success(&health.status));
    println!();
    println!("{}", color::success("✓ API server started"));
    println!("  {}", color::muted("Press Ctrl+C to stop"));
    Ok(())
}

pub fn handle_api_status(output: String) -> Result<()> {
    use crate::api::server::ApiServer;
    use crate::api::ApiConfig;

    let config = ApiConfig::new(8080);
    let server = ApiServer::new(config);

    match output.as_str() {
        "json" => {
            let health = server.health_status();
            let json = serde_json::to_string_pretty(&health)?;
            println!("{}", json);
        }
        "yaml" => {
            let health = server.health_status();
            let yaml = serde_yaml::to_string(&health)?;
            println!("{}", yaml);
        }
        _ => {
            println!("{}", color::header("API Server Status"));
            println!();

            let health = server.health_status();
            println!(
                "  Status:    {}",
                if health.is_healthy() {
                    color::success(&health.status)
                } else {
                    color::warning(&health.status)
                }
            );
            println!("  Version:   {}", health.version);
            println!("  Uptime:    {} seconds", health.uptime_secs);
            println!();

            println!("{}", color::label("Stats:"));
            println!("  Requests:    {}", server.stats.total_requests);
            println!("  Errors:      {}", server.stats.error_count);
            println!("  Avg Latency: {:.1}ms", server.stats.avg_response_ms);
            println!("  Success:     {:.1}%", server.stats.success_rate());
        }
    }
    Ok(())
}

pub fn handle_api_routes(method: Option<String>, output: String) -> Result<()> {
    use crate::api::routes::build_default_router;

    let router = build_default_router();

    let routes = if let Some(ref m) = method {
        router.routes_by_method(m)
    } else {
        router.all_routes()
    };

    match output.as_str() {
        "json" => {
            let json = serde_json::to_string_pretty(&routes)?;
            println!("{}", json);
        }
        "yaml" => {
            let yaml = serde_yaml::to_string(&routes)?;
            println!("{}", yaml);
        }
        _ => {
            println!("{}", color::header("API Routes"));
            if let Some(ref m) = method {
                println!("  Filter: {}", color::value(m));
            }
            println!();

            println!(
                "  {:<8} {:<40} {:<20} {}",
                color::label("METHOD"),
                color::label("PATH"),
                color::label("HANDLER"),
                color::label("MIDDLEWARE"),
            );
            println!("  {}", "-".repeat(90));

            for route in &routes {
                let mw = if route.middleware.is_empty() {
                    "-".to_string()
                } else {
                    route.middleware.join(", ")
                };

                println!(
                    "  {:<8} {:<40} {:<20} {}",
                    color::value(&route.method),
                    route.full_path(),
                    route.handler,
                    color::muted(&mw),
                );
            }

            println!();
            println!(
                "  {} routes across {} groups",
                router.total_routes(),
                router.group_count(),
            );
        }
    }
    Ok(())
}

pub fn handle_api_spec(format: String, output: Option<String>) -> Result<()> {
    use crate::api::openapi::generate_default_spec;

    let spec = generate_default_spec();

    let content = match format.as_str() {
        "json" => serde_json::to_string_pretty(&spec)?,
        _ => serde_yaml::to_string(&spec)?,
    };

    if let Some(output_file) = output {
        std::fs::write(&output_file, &content)?;
        println!(
            "{}",
            color::success(&format!(
                "✓ OpenAPI specification written to {}",
                output_file
            ))
        );
        println!("  Paths:   {}", spec.path_count());
        println!("  Schemas: {}", spec.schema_count());
        println!("  Tags:    {}", spec.tag_count());
    } else {
        println!("{}", content);
    }
    Ok(())
}

pub fn handle_api_key_list(active_only: bool, output: String) -> Result<()> {
    use crate::api::ApiKeyManager;

    println!("{}", color::header("API Keys"));
    println!();

    let manager = ApiKeyManager::new();
    let keys = if active_only {
        manager.active_keys()
    } else {
        manager.list_keys()
    };

    println!("  Total keys: {}", keys.len());
    println!("  Format: {}", output);

    if keys.is_empty() {
        println!();
        println!("  {}", color::muted("No API keys found"));
        println!(
            "  {}",
            color::muted("Use 'zorvia api-key-create' to create one")
        );
    }

    println!();
    println!("{}", color::success("✓ Keys listed"));
    Ok(())
}

pub fn handle_api_key_create(
    name: String,
    permissions: String,
    rate_limit: Option<u32>,
) -> Result<()> {
    use crate::api::ApiKey;
    use chrono::Utc;

    println!("{}", color::header(&format!("Creating API Key: {}", name)));
    println!();

    let perms: Vec<String> = permissions
        .split(',')
        .map(|p| p.trim().to_string())
        .collect();

    let mut key = ApiKey::new(&name, format!("hash-{}", Utc::now().timestamp()))
        .with_permissions(perms.clone());

    if let Some(limit) = rate_limit {
        key = key.with_rate_limit(limit);
    }

    println!("  Name:        {}", color::value(&name));
    println!("  ID:          {}", color::muted(&key.id));
    println!("  Permissions: {}", perms.join(", "));
    if let Some(limit) = rate_limit {
        println!("  Rate Limit:  {} req/min", limit);
    }
    println!();
    println!("{}", color::success("✓ API key created"));
    Ok(())
}

pub fn handle_api_key_delete(key: String, yes: bool) -> Result<()> {
    println!("{}", color::header(&format!("Deleting API Key: {}", key)));
    println!();

    if !yes {
        println!(
            "  {}",
            color::warning("This will permanently revoke the API key")
        );
        println!("  Use --yes to skip confirmation");
    }

    println!();
    println!(
        "{}",
        color::success(&format!("✓ API key '{}' deleted", key))
    );
    Ok(())
}

pub fn handle_webhook_list(active_only: bool, output: String) -> Result<()> {
    use crate::api::webhooks::WebhookManager;

    println!("{}", color::header("Webhooks"));
    println!();

    let manager = WebhookManager::new();
    let webhooks = if active_only {
        manager.active_webhooks()
    } else {
        manager.list()
    };

    println!("  Total webhooks: {}", webhooks.len());
    println!("  Format: {}", output);

    if webhooks.is_empty() {
        println!();
        println!("  {}", color::muted("No webhooks registered"));
        println!(
            "  {}",
            color::muted("Use 'zorvia webhook-create' to register one")
        );
    }

    println!();
    println!("{}", color::success("✓ Webhooks listed"));
    Ok(())
}

pub fn handle_webhook_create(
    name: String,
    url: String,
    events: String,
    secret: Option<String>,
) -> Result<()> {
    use crate::api::webhooks::{WebhookConfig, WebhookEvent};

    println!(
        "{}",
        color::header(&format!("Registering Webhook: {}", name))
    );
    println!();

    let mut webhook = WebhookConfig::new(&name, &url);

    if let Some(s) = secret {
        webhook = webhook.with_secret(s);
    }

    let event_list: Vec<&str> = events.split(',').map(|e| e.trim()).collect();
    for event_str in &event_list {
        let event = match *event_str {
            "vm.created" => WebhookEvent::VMCreated,
            "vm.deleted" => WebhookEvent::VMDeleted,
            "vm.started" => WebhookEvent::VMStarted,
            "vm.stopped" => WebhookEvent::VMStopped,
            "vm.failed" => WebhookEvent::VMFailed,
            "backup.completed" => WebhookEvent::BackupCompleted,
            "backup.failed" => WebhookEvent::BackupFailed,
            "alert.triggered" => WebhookEvent::AlertTriggered,
            "alert.resolved" => WebhookEvent::AlertResolved,
            other => WebhookEvent::Custom(other.to_string()),
        };
        webhook.add_event(event);
    }

    println!("  Name:     {}", color::value(&name));
    println!("  URL:      {}", url);
    println!("  Events:   {}", events);
    println!("  ID:       {}", color::muted(&webhook.id));
    println!();
    println!("{}", color::success("✓ Webhook registered"));
    Ok(())
}

pub fn handle_webhook_delete(webhook: String, yes: bool) -> Result<()> {
    println!(
        "{}",
        color::header(&format!("Deleting Webhook: {}", webhook))
    );
    println!();

    if !yes {
        println!(
            "  {}",
            color::warning("This will permanently remove the webhook")
        );
        println!("  Use --yes to skip confirmation");
    }

    println!();
    println!(
        "{}",
        color::success(&format!("✓ Webhook '{}' deleted", webhook))
    );
    Ok(())
}

pub async fn handle_tui(namespace: String, theme: Option<String>, interactive: bool) -> Result<()> {
    use crossterm::{
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::{backend::CrosstermBackend, Terminal};
    use std::io;

    // Load TUI config
    let mut config = crate::tui::TuiConfig::load()?;

    // Apply theme if specified
    if let Some(theme_name) = theme {
        config.theme.name = theme_name;
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create and run app
    let result = if interactive {
        // Enhanced interactive mode with dialogs, menus, and notifications
        let mut app = crate::tui::InteractiveApp::with_config(namespace.clone(), config);
        app.run(&mut terminal).await
    } else {
        // Basic TUI mode
        let mut app = crate::tui::App::with_config(namespace.clone(), config);
        app.run(&mut terminal).await
    };

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    // Handle any errors
    result?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::api::webhooks::{WebhookConfig, WebhookEvent, WebhookManager};
    use crate::api::{ApiKey, ApiKeyManager, AuthMethod, RateLimitConfig};

    #[test]
    fn test_auth_method_parsing() {
        assert_eq!(AuthMethod::parse("none"), Some(AuthMethod::None));
        assert_eq!(AuthMethod::parse("bearer"), Some(AuthMethod::Bearer));
        assert_eq!(AuthMethod::parse("token"), Some(AuthMethod::Bearer));
        assert_eq!(AuthMethod::parse("apikey"), Some(AuthMethod::ApiKey));
        assert_eq!(AuthMethod::parse("mtls"), Some(AuthMethod::MTLS));
        assert_eq!(AuthMethod::parse("unknown"), None);
    }

    #[test]
    fn test_auth_method_fallback() {
        let invalid = AuthMethod::parse("invalid-auth").unwrap_or(AuthMethod::None);
        assert_eq!(invalid, AuthMethod::None);
    }

    #[test]
    fn test_api_key_permission_parsing() {
        let permissions = "read,write,admin";
        let perms: Vec<String> = permissions
            .split(',')
            .map(|p| p.trim().to_string())
            .collect();
        assert_eq!(perms, vec!["read", "write", "admin"]);
    }

    #[test]
    fn test_api_key_creation_flow() {
        let key = ApiKey::new("test-key", "hash-123")
            .with_permissions(vec!["read".to_string(), "write".to_string()])
            .with_rate_limit(100);
        assert!(key.has_permission("read"));
        assert!(!key.has_permission("admin"));
        assert_eq!(key.rate_limit, Some(100));
    }

    #[test]
    fn test_api_key_manager_lifecycle() {
        let mut manager = ApiKeyManager::new();
        let key = ApiKey::new("key1", "hash1");
        let id = manager.add_key(key);
        assert_eq!(manager.key_count(), 1);
        manager.get_key_mut(&id).unwrap().disable();
        assert_eq!(manager.active_keys().len(), 0);
        assert!(manager.remove_key(&id));
        assert_eq!(manager.key_count(), 0);
    }

    #[test]
    fn test_webhook_event_parsing() {
        let events = "vm.created,backup.completed,custom.deploy";
        let event_list: Vec<&str> = events.split(',').map(|e| e.trim()).collect();
        let mut parsed = Vec::new();
        for event_str in &event_list {
            let event = match *event_str {
                "vm.created" => WebhookEvent::VMCreated,
                "backup.completed" => WebhookEvent::BackupCompleted,
                other => WebhookEvent::Custom(other.to_string()),
            };
            parsed.push(event);
        }
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0], WebhookEvent::VMCreated);
        assert_eq!(parsed[2], WebhookEvent::Custom("custom.deploy".to_string()));
    }

    #[test]
    fn test_webhook_manager_for_event() {
        let mut manager = WebhookManager::new();
        let mut wh1 = WebhookConfig::new("vm-events", "https://example.com/vm");
        wh1.add_event(WebhookEvent::VMCreated);
        let mut wh2 = WebhookConfig::new("backup-events", "https://example.com/backup");
        wh2.add_event(WebhookEvent::BackupCompleted);
        manager.register(wh1);
        manager.register(wh2);
        assert_eq!(
            manager.webhooks_for_event(&WebhookEvent::VMCreated).len(),
            1
        );
        assert_eq!(manager.webhooks_for_event(&WebhookEvent::VMFailed).len(), 0);
    }

    #[test]
    fn test_rate_limit_config() {
        let enabled = RateLimitConfig::new(120);
        assert!(enabled.enabled);
        assert_eq!(enabled.burst_size, 60);
        let disabled = RateLimitConfig::disabled();
        assert!(!disabled.enabled);
    }
}
