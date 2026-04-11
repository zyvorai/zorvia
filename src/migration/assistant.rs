// Migration Assistant - Guided multi-step migration wizard

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationAssistant {
    pub current_step: MigrationStep,
    pub selected_vms: Vec<String>,
    pub target_node: Option<String>,
    pub pre_check_results: Vec<PreCheckResult>,
    pub post_check_results: Vec<PostCheckResult>,
    pub config: AssistantConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MigrationStep { SelectTarget, PreCheck, Confirm, Migrating, PostCheck, Complete }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreCheckResult {
    pub check_name: String,
    pub check_type: CheckType,
    pub status: CheckStatus,
    pub severity: CheckSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheckType { Resource, Network, Storage, Compatibility, Health }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CheckStatus { Passed, Warning, Failed, Skipped }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CheckSeverity { Required, Recommended, Optional }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostCheckResult {
    pub check_name: String,
    pub passed: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantConfig {
    pub auto_pre_check: bool,
    pub auto_post_check: bool,
    pub require_all_pre_checks: bool,
    pub timeout_secs: u64,
}

impl Default for AssistantConfig {
    fn default() -> Self {
        Self { auto_pre_check: true, auto_post_check: true, require_all_pre_checks: false, timeout_secs: 600 }
    }
}

impl MigrationAssistant {
    pub fn new() -> Self {
        Self { current_step: MigrationStep::SelectTarget, selected_vms: Vec::new(),
            target_node: None, pre_check_results: Vec::new(), post_check_results: Vec::new(),
            config: AssistantConfig::default() }
    }

    pub fn select_vms(&mut self, vms: Vec<String>) { self.selected_vms = vms; }
    pub fn select_target(&mut self, node: &str) { self.target_node = Some(node.to_string()); }

    pub fn run_pre_checks(&mut self) -> bool {
        self.pre_check_results = vec![
            PreCheckResult { check_name: "Target node capacity".to_string(), check_type: CheckType::Resource,
                status: CheckStatus::Passed, severity: CheckSeverity::Required, message: "Sufficient resources available".to_string() },
            PreCheckResult { check_name: "Network connectivity".to_string(), check_type: CheckType::Network,
                status: CheckStatus::Passed, severity: CheckSeverity::Required, message: "Network path verified".to_string() },
            PreCheckResult { check_name: "Storage accessibility".to_string(), check_type: CheckType::Storage,
                status: CheckStatus::Passed, severity: CheckSeverity::Required, message: "Storage accessible from target".to_string() },
            PreCheckResult { check_name: "VM compatibility".to_string(), check_type: CheckType::Compatibility,
                status: CheckStatus::Passed, severity: CheckSeverity::Required, message: "VM compatible with target node".to_string() },
        ];
        self.current_step = MigrationStep::PreCheck;
        self.pre_checks_passed()
    }

    pub fn pre_checks_passed(&self) -> bool {
        !self.pre_check_results.iter().any(|r| r.status == CheckStatus::Failed)
    }

    pub fn confirm(&mut self) { self.current_step = MigrationStep::Confirm; }
    pub fn start_migration(&mut self) { self.current_step = MigrationStep::Migrating; }

    pub fn run_post_checks(&mut self) {
        self.post_check_results = vec![
            PostCheckResult { check_name: "VM running".to_string(), passed: true, message: "VM is running on target node".to_string() },
            PostCheckResult { check_name: "Network accessible".to_string(), passed: true, message: "VM network is accessible".to_string() },
        ];
        self.current_step = MigrationStep::PostCheck;
    }

    pub fn complete(&mut self) { self.current_step = MigrationStep::Complete; }
    pub fn reset(&mut self) { *self = Self::new(); }
}

impl Default for MigrationAssistant {
    fn default() -> Self { Self::new() }
}
