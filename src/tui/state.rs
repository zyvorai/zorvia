// TUI State Management - Application state and data

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// VM information for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmInfo {
    pub name: String,
    pub status: String,
    pub cpu: String,
    pub memory: String,
    pub age: String,
    pub ready: bool,
}

impl VmInfo {
    pub fn from_vm(vm: &crate::kube::types::VirtualMachine) -> Self {
        let name = vm.metadata.name.clone().unwrap_or_default();

        let status = vm.status.as_ref()
            .and_then(|s| s.print_able_status.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        let ready = vm.status.as_ref()
            .and_then(|s| s.ready)
            .unwrap_or(false);

        // Extract CPU and memory from spec
        let cpu = vm.spec.template.spec.domain.cpu.as_ref()
            .map(|c| format!("{} cores", c.cores.unwrap_or(1)))
            .unwrap_or_else(|| "1 core".to_string());

        let memory = vm.spec.template.spec.domain.resources.requests.as_ref()
            .and_then(|req| req.get("memory"))
            .cloned()
            .unwrap_or_else(|| "Unknown".to_string());

        // Calculate age
        let age = vm.metadata.creation_timestamp.as_ref()
            .map(|created| {
                let now = Utc::now();
                let duration = now.signed_duration_since(created.0);

                let days = duration.num_days();
                let hours = duration.num_hours() % 24;
                let minutes = duration.num_minutes() % 60;

                if days > 0 {
                    format!("{}d{}h", days, hours)
                } else if hours > 0 {
                    format!("{}h{}m", hours, minutes)
                } else {
                    format!("{}m", minutes)
                }
            })
            .unwrap_or_else(|| "Unknown".to_string());

        Self {
            name,
            status,
            cpu,
            memory,
            age,
            ready,
        }
    }
}

/// Snapshot information for display
#[derive(Debug, Clone)]
pub struct SnapshotDisplayInfo {
    pub name: String,
    pub vm_name: String,
    pub status: String,
    pub age: String,
    pub ready: bool,
}

/// Application state
pub struct AppState {
    /// Current namespace
    pub namespace: String,

    /// List of VMs
    pub vms: Vec<VmInfo>,

    /// List of snapshots
    pub snapshots: Vec<SnapshotDisplayInfo>,

    /// Selected index in current list
    pub selected_index: usize,

    /// Last refresh time
    pub last_refresh: DateTime<Utc>,

    /// Auto-refresh interval in seconds
    pub refresh_interval: u64,
}

impl AppState {
    /// Create a new application state
    pub fn new(namespace: String) -> Self {
        Self {
            namespace,
            vms: Vec::new(),
            snapshots: Vec::new(),
            selected_index: 0,
            last_refresh: Utc::now(),
            refresh_interval: 5, // 5 seconds
        }
    }

    /// Refresh VMs from Kubernetes
    pub async fn refresh_vms(&mut self) -> Result<()> {
        use crate::kube::KubeClient;

        let client = KubeClient::new().await?;
        let vm_list = client.list_vms(&self.namespace).await?;

        self.vms = vm_list
            .into_iter()
            .map(|vm| VmInfo::from_vm(&vm))
            .collect();

        self.last_refresh = Utc::now();

        // Reset selection if out of bounds
        if self.selected_index >= self.vms.len() && !self.vms.is_empty() {
            self.selected_index = self.vms.len() - 1;
        }

        Ok(())
    }

    /// Refresh snapshots from Kubernetes
    pub async fn refresh_snapshots(&mut self) -> Result<()> {
        use crate::snapshots::SnapshotManager;

        let manager = SnapshotManager::new(&self.namespace).await?;
        let snapshot_list = manager.list_all_snapshots().await?;

        self.snapshots = snapshot_list
            .into_iter()
            .map(|s| {
                let age = s.age();
                SnapshotDisplayInfo {
                    name: s.name,
                    vm_name: s.vm_name,
                    status: s.status.to_string(),
                    age,
                    ready: s.ready_to_use,
                }
            })
            .collect();

        Ok(())
    }

    /// Select next item in the list
    pub fn select_next(&mut self) {
        if self.vms.is_empty() {
            return;
        }

        if self.selected_index < self.vms.len() - 1 {
            self.selected_index += 1;
        } else {
            self.selected_index = 0; // Wrap around
        }
    }

    /// Select previous item in the list
    pub fn select_previous(&mut self) {
        if self.vms.is_empty() {
            return;
        }

        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else {
            self.selected_index = self.vms.len() - 1; // Wrap around
        }
    }

    /// Get the currently selected VM
    pub fn selected_vm(&self) -> Option<&VmInfo> {
        self.vms.get(self.selected_index)
    }

    /// Check if data should be refreshed
    pub fn should_refresh(&self) -> bool {
        let elapsed = Utc::now()
            .signed_duration_since(self.last_refresh)
            .num_seconds();

        elapsed >= self.refresh_interval as i64
    }

    /// Get VM statistics
    pub fn get_stats(&self) -> VmStats {
        let total = self.vms.len();
        let running = self.vms.iter().filter(|vm| vm.status == "Running").count();
        let stopped = self.vms.iter().filter(|vm| vm.status == "Stopped").count();

        VmStats {
            total,
            running,
            stopped,
        }
    }
}

/// VM statistics
#[derive(Debug, Clone)]
pub struct VmStats {
    pub total: usize,
    pub running: usize,
    pub stopped: usize,
}
