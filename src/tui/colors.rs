// CLI color output helpers
// These are used for terminal output in the CLI (not TUI)

use colored::Colorize;
use owo_colors::OwoColorize;

/// Color scheme for CLI output
pub mod cli {
    use super::*;

    /// Format VM name
    pub fn vm_name(name: &str) -> String {
        name.bright_white().bold().to_string()
    }

    /// Format namespace
    pub fn namespace(ns: &str) -> String {
        if ns == "default" {
            ns.bright_blue().to_string()
        } else if ns.starts_with("kube-") {
            ns.bright_red().to_string()
        } else {
            ns.bright_green().to_string()
        }
    }

    /// Format VM status
    pub fn vm_status(status: &str) -> String {
        match status.to_lowercase().as_str() {
            "running" => status.bright_green().bold().to_string(),
            "pending" | "scheduling" => status.bright_yellow().to_string(),
            "stopped" | "terminated" => status.bright_black().to_string(),
            "failed" | "error" => status.bright_red().bold().to_string(),
            "migrating" => status.bright_cyan().to_string(),
            "paused" => status.bright_magenta().to_string(),
            _ => status.dimmed().to_string(),
        }
    }

    /// Format success message
    pub fn success(msg: &str) -> String {
        format!("{} {}", "✓".bright_green().bold(), msg.bright_green())
    }

    /// Format error message
    pub fn error(msg: &str) -> String {
        format!("{} {}", "✗".bright_red().bold(), msg.bright_red())
    }

    /// Format warning message
    pub fn warning(msg: &str) -> String {
        format!("{} {}", "⚠".bright_yellow().bold(), msg.bright_yellow())
    }

    /// Format info message
    pub fn info(msg: &str) -> String {
        format!("{} {}", "ℹ".bright_blue().bold(), msg.bright_blue())
    }

    /// Format resource value (CPU, Memory, etc.)
    pub fn resource(value: &str, resource_type: &str) -> String {
        match resource_type {
            "cpu" => value.bright_green().to_string(),
            "memory" => value.bright_blue().to_string(),
            "disk" => value.bright_magenta().to_string(),
            "network" => value.bright_cyan().to_string(),
            _ => value.white().to_string(),
        }
    }

    /// Format header text
    pub fn header(text: &str) -> String {
        text.bright_purple().bold().to_string()
    }

    /// Format label
    pub fn label(text: &str) -> String {
        text.bright_white().to_string()
    }

    /// Format value
    pub fn value(text: &str) -> String {
        text.white().bold().to_string()
    }

    /// Format muted/dimmed text
    pub fn muted(text: &str) -> String {
        text.dimmed().to_string()
    }

    /// Format command/code
    pub fn command(text: &str) -> String {
        text.bright_cyan().to_string()
    }

    /// Format file path
    pub fn path(text: &str) -> String {
        text.bright_yellow().to_string()
    }
}

/// VM status symbols with colors
pub fn vm_status_symbol(status: &str) -> String {
    match status.to_lowercase().as_str() {
        "running" => "●".bright_green().to_string(),
        "pending" | "scheduling" => "◐".bright_yellow().to_string(),
        "stopped" | "terminated" => "○".bright_black().to_string(),
        "failed" | "error" => "✗".bright_red().to_string(),
        "migrating" => "⟳".bright_cyan().to_string(),
        "paused" => "⏸".bright_magenta().to_string(),
        _ => "?".dimmed().to_string(),
    }
}

/// Create a colored bar for resource usage
pub fn resource_bar(percentage: f64, width: usize) -> String {
    let filled = ((percentage / 100.0) * width as f64) as usize;
    let empty = width.saturating_sub(filled);

    let color = if percentage < 70.0 {
        "green"
    } else if percentage < 90.0 {
        "yellow"
    } else {
        "red"
    };

    let filled_str = "█".repeat(filled);
    let empty_str = "░".repeat(empty);

    match color {
        "green" => format!("{}{}", filled_str.bright_green(), empty_str.bright_black()),
        "yellow" => format!("{}{}", filled_str.bright_yellow(), empty_str.bright_black()),
        "red" => format!("{}{}", filled_str.bright_red(), empty_str.bright_black()),
        _ => format!("{}{}", filled_str, empty_str),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_status_symbol() {
        let symbol = vm_status_symbol("running");
        assert!(symbol.contains("●"));

        let symbol = vm_status_symbol("failed");
        assert!(symbol.contains("✗"));
    }

    #[test]
    fn test_resource_bar() {
        let bar = resource_bar(50.0, 10);
        // Should have some filled and some empty chars
        assert!(!bar.is_empty());

        let bar = resource_bar(100.0, 10);
        assert!(!bar.is_empty());
    }
}
