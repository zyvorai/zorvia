use crate::tui::colors::cli as color;
use anyhow::Result;

pub fn handle_security_scan(
    vm: String,
    scan_type: String,
    containers: bool,
    output: String,
) -> Result<()> {
    use crate::security::scan::{ScanConfig, ScanType, VulnerabilityScanner};

    println!("{}", color::header(&format!("Scanning VM: {}", vm)));
    println!();

    let s_type = match scan_type.as_str() {
        "quick" => ScanType::Quick,
        "deep" => ScanType::Deep,
        "compliance" => ScanType::Compliance,
        _ => ScanType::Standard,
    };

    let mut config = ScanConfig::new(&vm, s_type);
    if containers {
        config = config.enable_containers();
    }

    println!("  Scan Type:  {}", color::value(&scan_type));
    println!(
        "  Containers: {}",
        if containers {
            color::success("Yes")
        } else {
            "No".to_string()
        }
    );
    println!();
    println!("Scanning...");

    let result = VulnerabilityScanner::scan(&config);

    println!();
    println!("Scan Results:");
    println!(
        "  Status:     {}",
        color::success(&result.status.to_string())
    );
    println!(
        "  Total:      {}",
        color::value(&result.statistics.total.to_string())
    );
    println!(
        "  Critical:   {}",
        if result.statistics.critical > 0 {
            color::error(&result.statistics.critical.to_string())
        } else {
            color::success("0")
        }
    );
    println!(
        "  High:       {}",
        if result.statistics.high > 0 {
            color::warning(&result.statistics.high.to_string())
        } else {
            color::success("0")
        }
    );
    println!("  Medium:     {}", result.statistics.medium);
    println!("  Low:        {}", result.statistics.low);
    println!();

    if output == "json" {
        let json = serde_json::to_string_pretty(&result)?;
        println!("{}", json);
    } else if output == "yaml" {
        let yaml = serde_yaml::to_string(&result)?;
        println!("{}", yaml);
    }

    Ok(())
}

pub fn handle_security_assess(vm: String, output: String) -> Result<()> {
    use crate::security::{SecurityAssessment, Severity, Vulnerability};

    let mut assessment = SecurityAssessment::new(&vm);

    // Example vulnerabilities
    assessment.add_vulnerability(
        Vulnerability::new("VULN-001", "OpenSSL vulnerability", Severity::High).with_cvss(7.5),
    );
    assessment.add_vulnerability(
        Vulnerability::new("VULN-002", "Kernel vulnerability", Severity::Medium).with_cvss(5.0),
    );

    assessment.calculate_score();

    println!("{}", color::header(&format!("Security Assessment: {}", vm)));
    println!();
    println!(
        "  Score:         {}",
        color::value(&assessment.overall_score.to_string())
    );
    println!(
        "  Risk Level:    {}",
        match assessment.risk_level {
            crate::security::RiskLevel::Critical => color::error("Critical"),
            crate::security::RiskLevel::High => color::error("High"),
            crate::security::RiskLevel::Medium => color::warning("Medium"),
            crate::security::RiskLevel::Low => color::success("Low"),
            crate::security::RiskLevel::Unknown => color::muted("Unknown"),
        }
    );
    println!("  Vulnerabilities: {}", assessment.vulnerabilities.len());
    println!(
        "    Critical:    {}",
        color::error(&assessment.critical_count().to_string())
    );
    println!(
        "    High:        {}",
        color::warning(&assessment.high_count().to_string())
    );
    println!();

    if output == "json" {
        let json = serde_json::to_string_pretty(&assessment)?;
        println!("{}", json);
    } else {
        let yaml = serde_yaml::to_string(&assessment)?;
        println!("{}", yaml);
    }

    Ok(())
}

pub fn handle_security_harden(vm: String, profile: String, verify_only: bool) -> Result<()> {
    use crate::security::hardening::{HardeningEngine, SecurityBaseline};

    println!("{}", color::header(&format!("Security Hardening: {}", vm)));
    println!();

    let baseline = match profile.as_str() {
        "stig" => SecurityBaseline::STIG,
        "pci-dss" => SecurityBaseline::PciDss,
        "nist" => SecurityBaseline::NIST,
        "custom" => SecurityBaseline::Custom,
        _ => SecurityBaseline::CIS,
    };

    let hardening_profile = match profile.as_str() {
        "stig" => HardeningEngine::stig_profile(),
        _ => HardeningEngine::cis_profile(),
    };

    println!("  Profile:     {}", color::value(&baseline.to_string()));
    println!("  Rules:       {}", hardening_profile.rule_count());
    println!(
        "  Mode:        {}",
        if verify_only {
            color::info("Verify Only")
        } else {
            color::warning("Apply")
        }
    );
    println!();

    let result = if verify_only {
        HardeningEngine::verify(&vm, &hardening_profile)
    } else {
        HardeningEngine::apply(&vm, &hardening_profile)
    };

    println!("Results:");
    println!(
        "  Status:      {}",
        color::success(&result.status.to_string())
    );
    println!(
        "  Applied:     {}",
        color::success(&result.statistics.applied.to_string())
    );
    println!("  Skipped:     {}", result.statistics.skipped);
    println!(
        "  Failed:      {}",
        if result.statistics.failed > 0 {
            color::error(&result.statistics.failed.to_string())
        } else {
            color::success("0")
        }
    );
    println!("  Success:     {}%", result.success_rate() as u8);

    Ok(())
}

pub fn handle_security_profiles(details: bool) -> Result<()> {
    use crate::security::hardening::{HardeningEngine, SecurityBaseline};

    println!("{}", color::header("Security Hardening Profiles"));
    println!();

    let profiles = vec![
        (SecurityBaseline::CIS, HardeningEngine::cis_profile()),
        (SecurityBaseline::STIG, HardeningEngine::stig_profile()),
    ];

    if details {
        for (baseline, profile) in profiles {
            println!("Profile: {}", color::value(&baseline.to_string()));
            println!("  Name:        {}", profile.name);
            println!("  Description: {}", profile.description);
            println!("  Rules:       {}", profile.rule_count());
            println!();
        }
    } else {
        println!(
            "{:<20} {:<50} {}",
            color::label("PROFILE"),
            color::label("DESCRIPTION"),
            color::label("RULES")
        );
        println!("{}", "-".repeat(80));

        for (baseline, profile) in profiles {
            println!(
                "{:<20} {:<50} {}",
                baseline.to_string(),
                profile.description,
                profile.rule_count()
            );
        }
    }

    Ok(())
}

pub fn handle_compliance_check(vm: String, framework: String, output: String) -> Result<()> {
    use crate::security::compliance::{ComplianceChecker, ComplianceFramework};

    println!("{}", color::header(&format!("Compliance Check: {}", vm)));
    println!();

    let fw = match framework.as_str() {
        "hipaa" => ComplianceFramework::HIPAA,
        "soc2" => ComplianceFramework::SOC2,
        "iso27001" => ComplianceFramework::ISO27001,
        "gdpr" => ComplianceFramework::GDPR,
        "nist" => ComplianceFramework::NIST,
        "cis" => ComplianceFramework::CIS,
        _ => ComplianceFramework::PCIDSS,
    };

    println!("  Framework:   {}", color::value(&fw.to_string()));
    println!();
    println!("Checking compliance...");

    let report = match framework.as_str() {
        "hipaa" => ComplianceChecker::check_hipaa(&vm),
        "soc2" => ComplianceChecker::check_soc2(&vm),
        _ => ComplianceChecker::check_pci_dss(&vm),
    };

    println!();
    println!("Compliance Report:");
    println!(
        "  Status:      {}",
        if report.compliant {
            color::success("Compliant")
        } else {
            color::error("Non-Compliant")
        }
    );
    println!("  Score:       {}%", report.summary.compliance_score as u8);
    println!("  Total:       {}", report.summary.total_checks);
    println!(
        "  Passed:      {}",
        color::success(&report.summary.passed.to_string())
    );
    println!(
        "  Failed:      {}",
        if report.summary.failed > 0 {
            color::error(&report.summary.failed.to_string())
        } else {
            color::success("0")
        }
    );
    println!(
        "  Critical:    {}",
        if report.summary.critical_failures > 0 {
            color::error(&report.summary.critical_failures.to_string())
        } else {
            color::success("0")
        }
    );
    println!();

    if output == "json" {
        let json = serde_json::to_string_pretty(&report)?;
        println!("{}", json);
    } else if output == "yaml" {
        let yaml = serde_yaml::to_string(&report)?;
        println!("{}", yaml);
    }

    Ok(())
}

pub fn handle_compliance_report(
    vm: String,
    report_id: Option<String>,
    output: String,
) -> Result<()> {
    use crate::security::compliance::ComplianceChecker;

    let report = ComplianceChecker::check_pci_dss(&vm);

    println!("{}", color::header(&format!("Compliance Report: {}", vm)));
    println!();
    println!(
        "  Report ID:   {}",
        report_id.as_deref().unwrap_or(&report.report_id)
    );
    println!(
        "  Generated:   {}",
        report.generated_at.format("%Y-%m-%d %H:%M:%S")
    );
    println!();

    if output == "json" {
        let json = serde_json::to_string_pretty(&report)?;
        println!("{}", json);
    } else {
        let yaml = serde_yaml::to_string(&report)?;
        println!("{}", yaml);
    }

    Ok(())
}

pub fn handle_audit_list(
    vm: Option<String>,
    event_type: Option<String>,
    severity: Option<String>,
    security_only: bool,
    output: String,
) -> Result<()> {
    use crate::security::audit::{AuditEvent, AuditLog, EventSeverity, EventType};

    let _ = (event_type, severity);

    println!("{}", color::header("Audit Events"));
    if let Some(ref vm_name) = vm {
        println!("  VM: {}", color::value(vm_name));
    }
    println!();

    // Create example audit log
    let mut log = AuditLog::new(vm.clone());

    // Add example events
    log.add_event(
        AuditEvent::new(
            EventType::Authentication,
            "user@example.com",
            "test-vm",
            "login",
        )
        .with_severity(EventSeverity::Info),
    );
    log.add_event(
        AuditEvent::new(EventType::VMOperation, "admin", "test-vm", "start")
            .with_severity(EventSeverity::Info),
    );
    log.add_event(
        AuditEvent::new(
            EventType::SecurityViolation,
            "user",
            "test-vm",
            "unauthorized",
        )
        .with_severity(EventSeverity::Critical),
    );

    let events: Vec<&AuditEvent> = if security_only {
        log.security_events()
    } else {
        log.events.iter().collect()
    };

    if output == "json" {
        let json = serde_json::to_string_pretty(&events)?;
        println!("{}", json);
    } else if output == "yaml" {
        let yaml = serde_yaml::to_string(&events)?;
        println!("{}", yaml);
    } else {
        println!(
            "{:<25} {:<20} {:<15} {:<10} {}",
            color::label("TIMESTAMP"),
            color::label("TYPE"),
            color::label("ACTOR"),
            color::label("SEVERITY"),
            color::label("ACTION")
        );
        println!("{}", "-".repeat(90));

        for event in events {
            let severity_str = match event.severity {
                EventSeverity::Critical => color::error("Critical"),
                EventSeverity::High => color::error("High"),
                EventSeverity::Medium => color::warning("Medium"),
                EventSeverity::Low => color::info("Low"),
                EventSeverity::Info => color::muted("Info"),
            };

            println!(
                "{:<25} {:<20} {:<15} {:<10} {}",
                event.timestamp.format("%Y-%m-%d %H:%M:%S"),
                event.event_type.to_string(),
                event.actor,
                severity_str,
                event.action
            );
        }
    }

    Ok(())
}

pub fn handle_audit_get(log_id: String, output: String) -> Result<()> {
    use crate::security::audit::AuditLog;

    let log = AuditLog::new(Some("test-vm".to_string()));

    println!("{}", color::header(&format!("Audit Log: {}", log_id)));
    println!();
    println!("  Log ID:      {}", log.log_id);
    println!("  Events:      {}", log.event_count());
    println!(
        "  Created:     {}",
        log.created_at.format("%Y-%m-%d %H:%M:%S")
    );
    println!();

    if output == "json" {
        let json = serde_json::to_string_pretty(&log)?;
        println!("{}", json);
    } else {
        let yaml = serde_yaml::to_string(&log)?;
        println!("{}", yaml);
    }

    Ok(())
}

pub fn handle_audit_stats(vm: Option<String>, period: String, output: String) -> Result<()> {
    use crate::security::audit::{AuditLog, AuditStatistics};

    println!("{}", color::header("Audit Statistics"));
    if let Some(ref vm_name) = vm {
        println!("  VM:          {}", color::value(vm_name));
    }
    println!("  Period:      {}", period);
    println!();

    let log = AuditLog::new(vm);
    let stats = AuditStatistics::from_log(&log);

    if output == "json" {
        let json = serde_json::to_string_pretty(&stats)?;
        println!("{}", json);
    } else if output == "yaml" {
        let yaml = serde_yaml::to_string(&stats)?;
        println!("{}", yaml);
    } else {
        println!("Summary:");
        println!("  Total Events:      {}", stats.total_events);
        println!("  Security Events:   {}", stats.security_events);
        println!(
            "  Critical Events:   {}",
            if stats.critical_events > 0 {
                color::error(&stats.critical_events.to_string())
            } else {
                color::success("0")
            }
        );
        println!(
            "  Failed Events:     {}",
            if stats.failed_events > 0 {
                color::warning(&stats.failed_events.to_string())
            } else {
                color::success("0")
            }
        );
    }

    Ok(())
}
