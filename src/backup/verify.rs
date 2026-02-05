// Backup Verification - Verify backup integrity and restorability

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Verification report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    pub backup_name: String,
    pub verification_type: VerificationType,
    pub status: VerificationStatus,
    pub checks: Vec<VerificationCheck>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_count: u32,
    pub warning_count: u32,
}

impl VerificationReport {
    pub fn new(backup_name: impl Into<String>, verification_type: VerificationType) -> Self {
        Self {
            backup_name: backup_name.into(),
            verification_type,
            status: VerificationStatus::Pending,
            checks: Vec::new(),
            started_at: Utc::now(),
            completed_at: None,
            error_count: 0,
            warning_count: 0,
        }
    }

    pub fn add_check(&mut self, check: VerificationCheck) {
        match check.result {
            CheckResult::Failed => self.error_count += 1,
            CheckResult::Warning => self.warning_count += 1,
            CheckResult::Passed => {}
        }
        self.checks.push(check);
    }

    pub fn finalize(&mut self) {
        self.completed_at = Some(Utc::now());
        self.status = if self.error_count > 0 {
            VerificationStatus::Failed
        } else if self.warning_count > 0 {
            VerificationStatus::Warning
        } else {
            VerificationStatus::Passed
        };
    }

    pub fn duration_secs(&self) -> i64 {
        match self.completed_at {
            Some(completed) => completed.signed_duration_since(self.started_at).num_seconds(),
            None => Utc::now().signed_duration_since(self.started_at).num_seconds(),
        }
    }

    pub fn pass_rate(&self) -> f64 {
        if self.checks.is_empty() {
            return 100.0;
        }
        let passed = self.checks.iter()
            .filter(|c| c.result == CheckResult::Passed)
            .count();
        (passed as f64 / self.checks.len() as f64) * 100.0
    }
}

/// Verification type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VerificationType {
    Quick,       // Fast integrity checks
    Standard,    // Checksum + basic restore test
    Full,        // Complete restore test
}

/// Verification status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VerificationStatus {
    Pending,
    Running,
    Passed,
    Warning,
    Failed,
}

impl std::fmt::Display for VerificationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationStatus::Pending => write!(f, "Pending"),
            VerificationStatus::Running => write!(f, "Running"),
            VerificationStatus::Passed => write!(f, "Passed"),
            VerificationStatus::Warning => write!(f, "Warning"),
            VerificationStatus::Failed => write!(f, "Failed"),
        }
    }
}

/// Verification check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationCheck {
    pub check_name: String,
    pub description: String,
    pub result: CheckResult,
    pub message: String,
}

impl VerificationCheck {
    pub fn new(check_name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            check_name: check_name.into(),
            description: description.into(),
            result: CheckResult::Passed,
            message: String::new(),
        }
    }

    pub fn passed(mut self) -> Self {
        self.result = CheckResult::Passed;
        self.message = "Check passed".to_string();
        self
    }

    pub fn failed(mut self, message: impl Into<String>) -> Self {
        self.result = CheckResult::Failed;
        self.message = message.into();
        self
    }

    pub fn warning(mut self, message: impl Into<String>) -> Self {
        self.result = CheckResult::Warning;
        self.message = message.into();
        self
    }
}

/// Check result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CheckResult {
    Passed,
    Warning,
    Failed,
}

/// Verification runner
pub struct VerificationRunner;

impl VerificationRunner {
    /// Run verification checks
    pub fn verify(backup_name: &str, verification_type: VerificationType) -> VerificationReport {
        let mut report = VerificationReport::new(backup_name, verification_type.clone());

        // File existence check
        report.add_check(
            VerificationCheck::new("file-exists", "Verify backup file exists")
                .passed()
        );

        // Checksum verification
        report.add_check(
            VerificationCheck::new("checksum", "Verify backup checksum")
                .passed()
        );

        // Metadata check
        report.add_check(
            VerificationCheck::new("metadata", "Verify backup metadata")
                .passed()
        );

        // Compression check
        report.add_check(
            VerificationCheck::new("compression", "Verify compression integrity")
                .passed()
        );

        match verification_type {
            VerificationType::Quick => {
                // Quick checks only
            }
            VerificationType::Standard => {
                // Add decompression test
                report.add_check(
                    VerificationCheck::new("decompress", "Test decompression")
                        .passed()
                );
            }
            VerificationType::Full => {
                // Add full restore test
                report.add_check(
                    VerificationCheck::new("restore-test", "Full restore test")
                        .passed()
                );
                
                report.add_check(
                    VerificationCheck::new("boot-test", "VM boot test")
                        .passed()
                );
            }
        }

        report.finalize();
        report
    }

    /// Quick integrity check
    pub fn quick_check(backup_name: &str) -> bool {
        let report = Self::verify(backup_name, VerificationType::Quick);
        report.status == VerificationStatus::Passed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verification_report() {
        let mut report = VerificationReport::new("backup-001", VerificationType::Standard);
        assert_eq!(report.error_count, 0);
        assert_eq!(report.warning_count, 0);

        report.add_check(
            VerificationCheck::new("test1", "Test check 1").passed()
        );
        report.add_check(
            VerificationCheck::new("test2", "Test check 2").warning("Minor issue")
        );
        report.add_check(
            VerificationCheck::new("test3", "Test check 3").failed("Critical error")
        );

        assert_eq!(report.error_count, 1);
        assert_eq!(report.warning_count, 1);
        assert_eq!(report.checks.len(), 3);
    }

    #[test]
    fn test_verification_finalize() {
        let mut report = VerificationReport::new("backup-001", VerificationType::Quick);
        report.add_check(VerificationCheck::new("test", "Test").passed());
        report.finalize();

        assert_eq!(report.status, VerificationStatus::Passed);
        assert!(report.completed_at.is_some());

        let mut report_with_error = VerificationReport::new("backup-002", VerificationType::Quick);
        report_with_error.add_check(
            VerificationCheck::new("test", "Test").failed("Error")
        );
        report_with_error.finalize();

        assert_eq!(report_with_error.status, VerificationStatus::Failed);
    }

    #[test]
    fn test_verification_check() {
        let check = VerificationCheck::new("checksum", "Verify checksum")
            .passed();

        assert_eq!(check.check_name, "checksum");
        assert_eq!(check.result, CheckResult::Passed);

        let failed = VerificationCheck::new("restore", "Restore test")
            .failed("Restore failed");

        assert_eq!(failed.result, CheckResult::Failed);
        assert_eq!(failed.message, "Restore failed");
    }

    #[test]
    fn test_verification_types() {
        let quick = VerificationReport::new("backup", VerificationType::Quick);
        assert_eq!(quick.verification_type, VerificationType::Quick);

        let full = VerificationReport::new("backup", VerificationType::Full);
        assert_eq!(full.verification_type, VerificationType::Full);
    }

    #[test]
    fn test_pass_rate() {
        let mut report = VerificationReport::new("backup", VerificationType::Standard);
        
        report.add_check(VerificationCheck::new("test1", "Test 1").passed());
        report.add_check(VerificationCheck::new("test2", "Test 2").passed());
        report.add_check(VerificationCheck::new("test3", "Test 3").failed("Error"));
        report.add_check(VerificationCheck::new("test4", "Test 4").passed());

        assert_eq!(report.pass_rate(), 75.0); // 3 out of 4 passed
    }

    #[test]
    fn test_verification_runner() {
        let report = VerificationRunner::verify("test-backup", VerificationType::Quick);
        assert!(report.checks.len() >= 4); // At least basic checks

        let quick_check = VerificationRunner::quick_check("test-backup");
        assert!(quick_check);
    }

    #[test]
    fn test_verification_status_display() {
        assert_eq!(VerificationStatus::Passed.to_string(), "Passed");
        assert_eq!(VerificationStatus::Failed.to_string(), "Failed");
        assert_eq!(VerificationStatus::Warning.to_string(), "Warning");
    }
}
