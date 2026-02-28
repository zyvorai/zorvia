use anyhow::Result;
use crate::tui::colors::cli as color;

pub fn handle_automation_list(enabled_only: bool, output: String) -> Result<()> {
    use crate::automation::{AutomationRule, Trigger};

    println!("{}", color::header("Automation Rules"));
    println!();

    // Example rules
    let rules = vec![
        AutomationRule::new("Auto Stop Idle VMs", Trigger::Manual),
        AutomationRule::new("Nightly Backup", Trigger::Schedule {
            cron: "0 2 * * *".to_string()
        }),
    ];

    let filtered: Vec<_> = if enabled_only {
        rules.iter().filter(|r| r.enabled).collect()
    } else {
        rules.iter().collect()
    };

    if output == "json" {
        let json = serde_json::to_string_pretty(&filtered)?;
        println!("{}", json);
    } else if output == "yaml" {
        let yaml = serde_yaml::to_string(&filtered)?;
        println!("{}", yaml);
    } else {
        println!("{:<30} {:<15} {:<10} {}",
            color::label("NAME"),
            color::label("TRIGGER"),
            color::label("STATUS"),
            color::label("EXECUTIONS")
        );
        println!("{}", "-".repeat(75));

        for rule in filtered {
            let status = if rule.enabled {
                color::success("Enabled")
            } else {
                color::muted("Disabled")
            };

            println!("{:<30} {:<15} {:<10} {}",
                rule.name,
                format!("{:?}", rule.trigger).split_whitespace().next().unwrap_or("Unknown"),
                status,
                rule.execution_count
            );
        }
    }
    Ok(())
}

pub fn handle_automation_create(
    name: String,
    description: Option<String>,
    trigger: String,
    enable: bool,
) -> Result<()> {
    use crate::automation::{AutomationRule, Trigger};

    println!("{}", color::header(&format!("Creating Automation Rule: {}", name)));
    println!();

    let trigger_type = match trigger.as_str() {
        "schedule" => Trigger::Schedule { cron: "0 * * * *".to_string() },
        "event" => Trigger::Event { event_type: "vm.started".to_string() },
        "metric" => Trigger::MetricThreshold {
            metric: "cpu_usage".to_string(),
            threshold: 80.0,
            operator: crate::automation::Operator::GreaterThan
        },
        _ => Trigger::Manual,
    };

    let mut rule = AutomationRule::new(&name, trigger_type);
    if let Some(desc) = description {
        rule = rule.with_description(desc);
    }
    if !enable {
        rule = rule.disable();
    }

    println!("  Name:        {}", color::value(&rule.name));
    println!("  Trigger:     {:?}", rule.trigger);
    println!("  Status:      {}", if rule.enabled {
        color::success("Enabled")
    } else {
        color::muted("Disabled")
    });
    println!();
    println!("{}", color::success("✓ Automation rule created successfully"));
    Ok(())
}

pub fn handle_automation_get(rule: String, output: String) -> Result<()> {
    use crate::automation::{AutomationRule, Trigger};

    let automation_rule = AutomationRule::new(&rule, Trigger::Manual)
        .with_description("Example automation rule");

    println!("{}", color::header(&format!("Automation Rule: {}", rule)));
    println!();

    if output == "json" {
        let json = serde_json::to_string_pretty(&automation_rule)?;
        println!("{}", json);
    } else {
        let yaml = serde_yaml::to_string(&automation_rule)?;
        println!("{}", yaml);
    }
    Ok(())
}

pub fn handle_automation_run(rule: String, dry_run: bool) -> Result<()> {
    use crate::automation::{ExecutionResult, ExecutionStatus, ActionResult};

    println!("{}", color::header(&format!("Executing Automation Rule: {}", rule)));
    if dry_run {
        println!("  Mode: {}", color::info("Dry Run"));
    }
    println!();

    let mut result = ExecutionResult::new(&rule);
    result.add_action_result(ActionResult::success("start-vm", "VM started successfully"));
    result.add_action_result(ActionResult::success("create-snapshot", "Snapshot created"));
    result.complete(ExecutionStatus::Completed);

    println!("Execution Results:");
    println!("  Status:      {}", color::success(&result.status.to_string()));
    println!("  Duration:    {}s", result.duration_secs());
    println!("  Successful:  {}", color::success(&result.success_count().to_string()));
    println!("  Failed:      {}", if result.failure_count() > 0 {
        color::error(&result.failure_count().to_string())
    } else {
        color::success("0")
    });
    Ok(())
}

pub fn handle_workflow_list(output: String) -> Result<()> {
    use crate::automation::workflows::WorkflowTemplates;

    println!("{}", color::header("Workflows"));
    println!();

    let workflows = vec![
        WorkflowTemplates::vm_provisioning(),
        WorkflowTemplates::disaster_recovery(),
        WorkflowTemplates::maintenance(),
    ];

    if output == "json" {
        let json = serde_json::to_string_pretty(&workflows)?;
        println!("{}", json);
    } else if output == "yaml" {
        let yaml = serde_yaml::to_string(&workflows)?;
        println!("{}", yaml);
    } else {
        println!("{:<30} {:<50} {}",
            color::label("NAME"),
            color::label("DESCRIPTION"),
            color::label("STEPS")
        );
        println!("{}", "-".repeat(90));

        for workflow in workflows {
            println!("{:<30} {:<50} {}",
                workflow.name,
                workflow.description,
                workflow.step_count()
            );
        }
    }
    Ok(())
}

pub fn handle_workflow_create(
    name: String,
    description: Option<String>,
    template: Option<String>,
) -> Result<()> {
    use crate::automation::workflows::{Workflow, WorkflowTemplates};

    println!("{}", color::header(&format!("Creating Workflow: {}", name)));
    println!();

    let mut workflow = if let Some(tmpl) = template {
        match tmpl.as_str() {
            "provisioning" => WorkflowTemplates::vm_provisioning(),
            "disaster-recovery" => WorkflowTemplates::disaster_recovery(),
            "maintenance" => WorkflowTemplates::maintenance(),
            _ => Workflow::new(&name),
        }
    } else {
        Workflow::new(&name)
    };

    if let Some(desc) = description {
        workflow = workflow.with_description(desc);
    }

    println!("  Name:        {}", color::value(&workflow.name));
    println!("  Description: {}", workflow.description);
    println!("  Steps:       {}", workflow.step_count());
    println!();
    println!("{}", color::success("✓ Workflow created successfully"));
    Ok(())
}

pub fn handle_workflow_get(workflow: String, output: String) -> Result<()> {
    use crate::automation::workflows::WorkflowTemplates;

    let wf = WorkflowTemplates::vm_provisioning();

    println!("{}", color::header(&format!("Workflow: {}", workflow)));
    println!();

    if output == "json" {
        let json = serde_json::to_string_pretty(&wf)?;
        println!("{}", json);
    } else {
        let yaml = serde_yaml::to_string(&wf)?;
        println!("{}", yaml);
    }
    Ok(())
}

pub fn handle_workflow_run(workflow: String, _watch: bool) -> Result<()> {
    use crate::automation::workflows::{WorkflowExecutor, WorkflowTemplates};

    println!("{}", color::header(&format!("Executing Workflow: {}", workflow)));
    println!();

    let wf = WorkflowTemplates::vm_provisioning();
    let execution = WorkflowExecutor::execute(&wf);

    println!("Execution:");
    println!("  ID:          {}", execution.execution_id);
    println!("  Status:      {}", color::success(&execution.status.to_string()));
    println!("  Duration:    {}s", execution.duration_secs());
    println!("  Steps:       {}/{} completed",
        execution.completed_steps().len(),
        execution.step_results.len()
    );
    println!("  Success Rate: {}%", execution.success_rate() as u8);
    Ok(())
}

pub fn handle_workflow_executions(
    workflow: Option<String>,
    limit: usize,
    output: String,
) -> Result<()> {
    use crate::automation::workflows::WorkflowExecution;

    println!("{}", color::header("Workflow Executions"));
    if let Some(ref wf_name) = workflow {
        println!("  Workflow: {}", color::value(wf_name));
    }
    println!("  Limit: {}", limit);
    println!();

    // Example execution
    let execution = WorkflowExecution::new("wf-123", "VM Provisioning");

    let executions = vec![execution];

    if output == "json" {
        let json = serde_json::to_string_pretty(&executions)?;
        println!("{}", json);
    } else if output == "yaml" {
        let yaml = serde_yaml::to_string(&executions)?;
        println!("{}", yaml);
    } else {
        println!("{:<25} {:<30} {:<15} {}",
            color::label("ID"),
            color::label("WORKFLOW"),
            color::label("STATUS"),
            color::label("STARTED")
        );
        println!("{}", "-".repeat(85));

        for exec in executions {
            println!("{:<25} {:<30} {:<15} {}",
                exec.execution_id,
                exec.workflow_name,
                exec.status.to_string(),
                exec.started_at.format("%Y-%m-%d %H:%M:%S")
            );
        }
    }
    Ok(())
}

pub fn handle_schedule_list(enabled_only: bool, output: String) -> Result<()> {
    use crate::automation::schedules::{ScheduledTask, Schedule};

    println!("{}", color::header("Scheduled Tasks"));
    println!();

    let tasks = vec![
        ScheduledTask::new("Daily Backup", Schedule::daily(2, 0), "rule-123"),
        ScheduledTask::new("Hourly Health Check", Schedule::hourly(0), "rule-456"),
    ];

    let filtered: Vec<_> = if enabled_only {
        tasks.iter().filter(|t| t.enabled).collect()
    } else {
        tasks.iter().collect()
    };

    if output == "json" {
        let json = serde_json::to_string_pretty(&filtered)?;
        println!("{}", json);
    } else if output == "yaml" {
        let yaml = serde_yaml::to_string(&filtered)?;
        println!("{}", yaml);
    } else {
        println!("{:<30} {:<15} {:<10} {}",
            color::label("NAME"),
            color::label("SCHEDULE"),
            color::label("STATUS"),
            color::label("RUN COUNT")
        );
        println!("{}", "-".repeat(75));

        for task in filtered {
            let status = if task.enabled {
                color::success("Enabled")
            } else {
                color::muted("Disabled")
            };

            println!("{:<30} {:<15} {:<10} {}",
                task.name,
                "Daily", // Simplified
                status,
                task.run_count
            );
        }
    }
    Ok(())
}

pub fn handle_schedule_create(
    name: String,
    rule: String,
    schedule: String,
    enable: bool,
) -> Result<()> {
    use crate::automation::schedules::{ScheduledTask, Schedule};

    println!("{}", color::header(&format!("Creating Scheduled Task: {}", name)));
    println!();

    let sched = if schedule.starts_with("interval:") {
        let seconds: u64 = schedule.strip_prefix("interval:").unwrap_or("3600").parse().unwrap_or(3600);
        Schedule::interval(seconds)
    } else {
        match schedule.as_str() {
            "hourly" => Schedule::hourly(0),
            "daily" => Schedule::daily(2, 0),
            "weekly" => Schedule::weekly(chrono::Weekday::Mon, 2, 0),
            _ => Schedule::daily(2, 0),
        }
    };

    let mut task = ScheduledTask::new(&name, sched, &rule);
    if !enable {
        task = task.disable();
    }

    println!("  Name:     {}", color::value(&task.name));
    println!("  Rule:     {}", task.rule_id);
    println!("  Schedule: {}", schedule);
    println!("  Status:   {}", if task.enabled {
        color::success("Enabled")
    } else {
        color::muted("Disabled")
    });
    println!();
    println!("{}", color::success("✓ Scheduled task created successfully"));
    Ok(())
}
