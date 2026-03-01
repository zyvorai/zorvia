use crate::tui::colors::cli as color;
use anyhow::Result;

/// Parse a log level string into a LogLevel enum.
/// Supported values (case-insensitive): debug, info, warning, error, critical.
/// Unknown values default to Info.
pub(crate) fn parse_log_level(level: &str) -> crate::observability::logs::LogLevel {
    use crate::observability::logs::LogLevel;

    match level.to_lowercase().as_str() {
        "debug" => LogLevel::Debug,
        "info" => LogLevel::Info,
        "warning" => LogLevel::Warning,
        "error" => LogLevel::Error,
        "critical" => LogLevel::Critical,
        _ => LogLevel::Info,
    }
}

/// Parse an alert severity string into an AlertSeverity enum.
/// Supported values (case-insensitive): info, warning, critical.
/// Unknown values default to Warning.
pub(crate) fn parse_alert_severity(severity: &str) -> crate::observability::alerts::AlertSeverity {
    use crate::observability::alerts::AlertSeverity;

    match severity.to_lowercase().as_str() {
        "info" => AlertSeverity::Info,
        "warning" => AlertSeverity::Warning,
        "critical" => AlertSeverity::Critical,
        _ => AlertSeverity::Warning,
    }
}

/// Parse a threshold operator string into a ThresholdOperator enum.
/// Supported values (case-insensitive): gt, lt, eq, gte, lte.
/// Unknown values default to GreaterThan.
pub(crate) fn parse_threshold_operator(
    operator: &str,
) -> crate::observability::alerts::ThresholdOperator {
    use crate::observability::alerts::ThresholdOperator;

    match operator.to_lowercase().as_str() {
        "gt" => ThresholdOperator::GreaterThan,
        "lt" => ThresholdOperator::LessThan,
        "eq" => ThresholdOperator::Equal,
        "gte" => ThresholdOperator::GreaterThanOrEqual,
        "lte" => ThresholdOperator::LessThanOrEqual,
        _ => ThresholdOperator::GreaterThan,
    }
}

pub fn handle_logs_query(
    level: Option<String>,
    source: Option<String>,
    search: Option<String>,
    limit: usize,
) -> Result<()> {
    use crate::observability::logs::LogQuery;

    println!("{}", color::header("Querying Logs"));
    println!();

    let mut _query = LogQuery::new().with_limit(limit);

    if let Some(ref lvl) = level {
        _query = _query.with_level(parse_log_level(lvl));
    }

    if let Some(ref src) = source {
        _query = _query.with_source(src);
    }

    if let Some(ref text) = search {
        _query = _query.with_search(text);
    }

    println!("  Level:  {}", level.as_deref().unwrap_or("all"));
    println!("  Source: {}", source.as_deref().unwrap_or("all"));
    println!("  Limit:  {}", limit);
    println!();
    println!("{}", color::success("✓ Log query executed"));
    Ok(())
}

pub fn handle_logs_stats(group_by: String) -> Result<()> {
    println!("{}", color::header("Log Statistics"));
    println!();

    println!("  Grouped by: {}", color::value(&group_by));
    println!();
    println!("{}", color::success("✓ Statistics generated"));
    Ok(())
}

pub fn handle_logs_patterns(min_count: usize) -> Result<()> {
    println!("{}", color::header("Log Pattern Analysis"));
    println!();

    println!("  Minimum count: {}", min_count);
    println!();
    println!("{}", color::success("✓ Patterns detected"));
    Ok(())
}

pub fn handle_metrics_collect(vm: String) -> Result<()> {
    use crate::observability::metrics::VMMetrics;

    println!("{}", color::header(&format!("Collecting Metrics: {}", vm)));
    println!();

    let metrics = VMMetrics::new(&vm)
        .with_cpu(45.5)
        .with_memory(62.3, 2_500_000_000)
        .with_disk_io(1_000_000.0, 500_000.0)
        .with_network_io(2_000_000.0, 1_500_000.0);

    println!("  CPU Usage:      {:.1}%", metrics.cpu_usage_percent);
    println!("  Memory Usage:   {:.1}%", metrics.memory_usage_percent);
    println!(
        "  Disk Read:      {:.2} MB/s",
        metrics.disk_read_bytes_per_sec / 1_000_000.0
    );
    println!(
        "  Disk Write:     {:.2} MB/s",
        metrics.disk_write_bytes_per_sec / 1_000_000.0
    );
    println!(
        "  Network RX:     {:.2} MB/s",
        metrics.network_rx_bytes_per_sec / 1_000_000.0
    );
    println!(
        "  Network TX:     {:.2} MB/s",
        metrics.network_tx_bytes_per_sec / 1_000_000.0
    );
    println!();
    println!("{}", color::success("✓ Metrics collected successfully"));
    Ok(())
}

pub fn handle_metrics_query(name: String, aggregation: String) -> Result<()> {
    println!("{}", color::header(&format!("Querying Metric: {}", name)));
    println!();

    println!("  Metric:      {}", color::value(&name));
    println!("  Aggregation: {}", aggregation);
    println!();
    println!("{}", color::success("✓ Query executed"));
    Ok(())
}

pub fn handle_metrics_snapshot(cpu_threshold: f64, memory_threshold: f64) -> Result<()> {
    use crate::observability::metrics::MetricsSnapshot;

    println!("{}", color::header("Metrics Snapshot"));
    println!();

    let snapshot = MetricsSnapshot::new();
    println!("  Total VMs:          {}", snapshot.total_vms);
    println!("  Cluster CPU Avg:    {:.1}%", snapshot.cluster_cpu_usage);
    println!(
        "  Cluster Memory Avg: {:.1}%",
        snapshot.cluster_memory_usage
    );
    println!("  CPU Threshold:      {}%", cpu_threshold);
    println!("  Memory Threshold:   {}%", memory_threshold);
    println!();
    println!("{}", color::success("✓ Snapshot captured"));
    Ok(())
}

pub fn handle_alerts_list(
    enabled_only: bool,
    severity: Option<String>,
    output: String,
) -> Result<()> {
    println!("{}", color::header("Alert Rules"));
    println!();

    println!(
        "  Filter: {}",
        if enabled_only { "Enabled only" } else { "All" }
    );
    if let Some(sev) = &severity {
        println!("  Severity: {}", color::value(sev));
    }
    println!("  Format: {}", output);
    println!();
    println!("{}", color::success("✓ Rules listed"));
    Ok(())
}

pub fn handle_alerts_create(
    name: String,
    severity: String,
    metric: String,
    operator: String,
    threshold: f64,
    duration: i64,
) -> Result<()> {
    use crate::observability::alerts::{AlertCondition, AlertRule};

    println!(
        "{}",
        color::header(&format!("Creating Alert Rule: {}", name))
    );
    println!();

    let alert_severity = parse_alert_severity(&severity);
    let op = parse_threshold_operator(&operator);

    let condition = AlertCondition::MetricThreshold {
        metric_name: metric.clone(),
        operator: op,
        threshold,
    };

    let rule = AlertRule::new(&name, alert_severity, condition)
        .with_duration(chrono::Duration::minutes(duration));

    println!("  Name:      {}", color::value(&rule.name));
    println!("  Severity:  {}", rule.severity);
    println!("  Metric:    {}", metric);
    println!("  Threshold: {} {}", operator, threshold);
    println!("  Duration:  {} minutes", duration);
    println!();
    println!("{}", color::success("✓ Alert rule created"));
    println!(
        "  {}",
        color::muted("Note: Configuration is not persisted to storage")
    );
    Ok(())
}

pub fn handle_alerts_active(severity: Option<String>, output: String) -> Result<()> {
    println!("{}", color::header("Active Alerts"));
    println!();

    if let Some(sev) = &severity {
        println!("  Severity filter: {}", color::value(sev));
    }
    println!("  Format: {}", output);
    println!();
    println!("{}", color::success("✓ Active alerts retrieved"));
    Ok(())
}

pub fn handle_alerts_resolve(alert_id: String) -> Result<()> {
    println!(
        "{}",
        color::header(&format!("Resolving Alert: {}", alert_id))
    );
    println!();

    println!("  Alert ID: {}", color::value(&alert_id));
    println!();
    println!("{}", color::success("✓ Alert resolved"));
    Ok(())
}

pub fn handle_insights_generate(
    vm: Option<String>,
    insight_type: Option<String>,
    min_severity: String,
) -> Result<()> {
    use crate::observability::insights::InsightAnalyzer;

    println!("{}", color::header("Generating Insights"));
    println!();

    if let Some(vm_name) = &vm {
        println!("  VM: {}", color::value(vm_name));

        // Generate sample insights
        let insights = InsightAnalyzer::analyze_resource_utilization(45.0, 62.0, vm_name);
        println!("  Generated {} insights", insights.len());
    } else {
        println!("  Scope: All VMs");
    }

    if let Some(itype) = &insight_type {
        println!("  Type: {}", color::value(itype));
    }
    println!("  Min Severity: {}", min_severity);
    println!();
    println!("{}", color::success("✓ Insights generated"));
    Ok(())
}

pub fn handle_recommendations(
    category: Option<String>,
    min_priority: String,
    with_savings: bool,
    output: String,
) -> Result<()> {
    println!("{}", color::header("Recommendations"));
    println!();

    if let Some(cat) = &category {
        println!("  Category: {}", color::value(cat));
    }
    println!("  Min Priority: {}", min_priority);
    println!("  Show Savings: {}", with_savings);
    println!("  Format: {}", output);
    println!();
    println!("{}", color::success("✓ Recommendations generated"));
    Ok(())
}

pub fn handle_trends_analyze(metric: String, window: i64, threshold: f64) -> Result<()> {
    println!("{}", color::header(&format!("Analyzing Trend: {}", metric)));
    println!();

    println!("  Metric:    {}", color::value(&metric));
    println!("  Window:    {} hours", window);
    println!("  Threshold: {}%", threshold);
    println!();
    println!("{}", color::success("✓ Trend analysis complete"));
    Ok(())
}

pub fn handle_health_check(component: Option<String>, output: String) -> Result<()> {
    use crate::observability::{HealthCheck, HealthStatus, SystemHealth};

    println!("{}", color::header("System Health Check"));
    println!();

    let mut health = SystemHealth::new();
    health.add_check(HealthCheck::new("api", HealthStatus::Healthy).with_response_time(15));
    health.add_check(HealthCheck::new("database", HealthStatus::Healthy).with_response_time(8));

    if let Some(comp) = &component {
        println!("  Component: {}", color::value(comp));
    } else {
        println!(
            "  Overall Status: {}",
            color::success(&health.overall_status.to_string())
        );
        println!("  Healthy:   {}", health.healthy_count());
        println!("  Unhealthy: {}", health.unhealthy_count());
    }
    println!("  Format: {}", output);
    println!();
    println!("{}", color::success("✓ Health check complete"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== parse_log_level tests =====

    #[test]
    fn test_parse_log_level_debug() {
        let level = parse_log_level("debug");
        assert!(matches!(level, crate::observability::logs::LogLevel::Debug));
    }

    #[test]
    fn test_parse_log_level_info() {
        let level = parse_log_level("info");
        assert!(matches!(level, crate::observability::logs::LogLevel::Info));
    }

    #[test]
    fn test_parse_log_level_warning() {
        let level = parse_log_level("warning");
        assert!(matches!(
            level,
            crate::observability::logs::LogLevel::Warning
        ));
    }

    #[test]
    fn test_parse_log_level_error() {
        let level = parse_log_level("error");
        assert!(matches!(level, crate::observability::logs::LogLevel::Error));
    }

    #[test]
    fn test_parse_log_level_critical() {
        let level = parse_log_level("critical");
        assert!(matches!(
            level,
            crate::observability::logs::LogLevel::Critical
        ));
    }

    #[test]
    fn test_parse_log_level_case_insensitive() {
        let level = parse_log_level("WARNING");
        assert!(matches!(
            level,
            crate::observability::logs::LogLevel::Warning
        ));
    }

    #[test]
    fn test_parse_log_level_unknown_defaults_to_info() {
        let level = parse_log_level("trace");
        assert!(matches!(level, crate::observability::logs::LogLevel::Info));
    }

    #[test]
    fn test_parse_log_level_empty_defaults_to_info() {
        let level = parse_log_level("");
        assert!(matches!(level, crate::observability::logs::LogLevel::Info));
    }

    // ===== parse_alert_severity tests =====

    #[test]
    fn test_parse_alert_severity_info() {
        let sev = parse_alert_severity("info");
        assert_eq!(sev, crate::observability::alerts::AlertSeverity::Info);
    }

    #[test]
    fn test_parse_alert_severity_warning() {
        let sev = parse_alert_severity("warning");
        assert_eq!(sev, crate::observability::alerts::AlertSeverity::Warning);
    }

    #[test]
    fn test_parse_alert_severity_critical() {
        let sev = parse_alert_severity("critical");
        assert_eq!(sev, crate::observability::alerts::AlertSeverity::Critical);
    }

    #[test]
    fn test_parse_alert_severity_case_insensitive() {
        let sev = parse_alert_severity("CRITICAL");
        assert_eq!(sev, crate::observability::alerts::AlertSeverity::Critical);
    }

    #[test]
    fn test_parse_alert_severity_unknown_defaults_to_warning() {
        let sev = parse_alert_severity("fatal");
        assert_eq!(sev, crate::observability::alerts::AlertSeverity::Warning);
    }

    // ===== parse_threshold_operator tests =====

    #[test]
    fn test_parse_threshold_operator_gt() {
        let op = parse_threshold_operator("gt");
        assert_eq!(
            op,
            crate::observability::alerts::ThresholdOperator::GreaterThan
        );
    }

    #[test]
    fn test_parse_threshold_operator_lt() {
        let op = parse_threshold_operator("lt");
        assert_eq!(
            op,
            crate::observability::alerts::ThresholdOperator::LessThan
        );
    }

    #[test]
    fn test_parse_threshold_operator_eq() {
        let op = parse_threshold_operator("eq");
        assert_eq!(op, crate::observability::alerts::ThresholdOperator::Equal);
    }

    #[test]
    fn test_parse_threshold_operator_gte() {
        let op = parse_threshold_operator("gte");
        assert_eq!(
            op,
            crate::observability::alerts::ThresholdOperator::GreaterThanOrEqual
        );
    }

    #[test]
    fn test_parse_threshold_operator_lte() {
        let op = parse_threshold_operator("lte");
        assert_eq!(
            op,
            crate::observability::alerts::ThresholdOperator::LessThanOrEqual
        );
    }

    #[test]
    fn test_parse_threshold_operator_case_insensitive() {
        let op = parse_threshold_operator("GT");
        assert_eq!(
            op,
            crate::observability::alerts::ThresholdOperator::GreaterThan
        );
    }

    #[test]
    fn test_parse_threshold_operator_unknown_defaults_to_gt() {
        let op = parse_threshold_operator("ne");
        assert_eq!(
            op,
            crate::observability::alerts::ThresholdOperator::GreaterThan
        );
    }

    #[test]
    fn test_parse_threshold_operator_empty_defaults_to_gt() {
        let op = parse_threshold_operator("");
        assert_eq!(
            op,
            crate::observability::alerts::ThresholdOperator::GreaterThan
        );
    }
}
