// Batch Migration - Bulk migration orchestration

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchMigration {
    pub id: String,
    pub name: String,
    pub items: Vec<BatchItem>,
    pub status: BatchStatus,
    pub config: BatchConfig,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchItem {
    pub vm_name: String,
    pub namespace: String,
    pub source_node: String,
    pub target_node: String,
    pub status: ItemStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BatchStatus { Planning, Ready, InProgress, Completed, PartiallyCompleted, Failed, Cancelled }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ItemStatus { Pending, InProgress, Completed, Failed, Skipped }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchConfig {
    pub max_parallel: usize,
    pub continue_on_error: bool,
    pub pause_between_secs: u64,
    pub dry_run: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self { max_parallel: 3, continue_on_error: true, pause_between_secs: 30, dry_run: false }
    }
}

impl BatchMigration {
    pub fn new(name: &str) -> Self {
        Self {
            id: format!("batch-{}", Utc::now().timestamp_micros()),
            name: name.to_string(), items: Vec::new(),
            status: BatchStatus::Planning, config: BatchConfig::default(),
            started_at: None, completed_at: None,
        }
    }

    pub fn add_item(&mut self, item: BatchItem) { self.items.push(item); }

    pub fn start(&mut self) { self.status = BatchStatus::InProgress; self.started_at = Some(Utc::now()); }

    pub fn complete(&mut self) {
        let all_done = self.items.iter().all(|i| matches!(i.status, ItemStatus::Completed | ItemStatus::Failed | ItemStatus::Skipped));
        if all_done {
            let any_failed = self.items.iter().any(|i| i.status == ItemStatus::Failed);
            self.status = if any_failed { BatchStatus::PartiallyCompleted } else { BatchStatus::Completed };
            self.completed_at = Some(Utc::now());
        }
    }

    pub fn cancel(&mut self) { self.status = BatchStatus::Cancelled; self.completed_at = Some(Utc::now()); }

    pub fn progress(&self) -> (usize, usize) {
        let done = self.items.iter().filter(|i| !matches!(i.status, ItemStatus::Pending | ItemStatus::InProgress)).count();
        (done, self.items.len())
    }

    pub fn success_count(&self) -> usize { self.items.iter().filter(|i| i.status == ItemStatus::Completed).count() }
    pub fn failed_count(&self) -> usize { self.items.iter().filter(|i| i.status == ItemStatus::Failed).count() }
}
