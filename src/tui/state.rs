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
    pub disk: String,
    pub ip: String,
    pub node: String,
}

impl VmInfo {
    pub fn from_vm(vm: &crate::kube::types::VirtualMachine) -> Self {
        let name = vm.metadata.name.clone().unwrap_or_default();

        let status = vm
            .status
            .as_ref()
            .and_then(|s| s.print_able_status.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        let ready = vm.status.as_ref().and_then(|s| s.ready).unwrap_or(false);

        // Extract CPU and memory from spec
        let cpu = vm
            .spec
            .template
            .spec
            .domain
            .cpu
            .as_ref()
            .map(|c| format!("{} cores", c.cores.unwrap_or(1)))
            .unwrap_or_else(|| "1 core".to_string());

        let memory = vm
            .spec
            .template
            .spec
            .domain
            .resources
            .requests
            .as_ref()
            .and_then(|req| req.get("memory"))
            .cloned()
            .unwrap_or_else(|| "Unknown".to_string());

        // Calculate age
        let age = vm
            .metadata
            .creation_timestamp
            .as_ref()
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

        // Extract disk info from volumes
        let disk = vm
            .spec
            .template
            .spec
            .volumes
            .as_ref()
            .map(|volumes| {
                // Sum up empty disk capacities, count PVCs
                let mut parts = Vec::new();
                for vol in volumes {
                    if let Some(ref empty) = vol.empty_disk {
                        parts.push(empty.capacity.clone());
                    } else if let Some(ref pvc) = vol.persistent_volume_claim {
                        parts.push(format!("pvc:{}", pvc.claim_name));
                    } else if let Some(ref dv) = vol.data_volume {
                        parts.push(format!("dv:{}", dv.name));
                    } else if vol.container_disk.is_some() {
                        parts.push("container".to_string());
                    }
                }
                if parts.is_empty() {
                    "None".to_string()
                } else {
                    parts.join(", ")
                }
            })
            .unwrap_or_else(|| "None".to_string());

        // Extract node name from conditions or status
        let node = vm
            .status
            .as_ref()
            .and_then(|s| {
                s.conditions.as_ref().and_then(|conds| {
                    conds
                        .iter()
                        .find(|c| c.type_ == "Ready" && c.status == "True")
                        .and_then(|c| c.message.clone())
                })
            })
            .unwrap_or_else(|| "N/A".to_string());

        // IP is not available in VM spec/status - needs VMI status
        let ip = "N/A".to_string();

        Self {
            name,
            status,
            cpu,
            memory,
            age,
            ready,
            disk,
            ip,
            node,
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

    /// Sort mode
    pub sort_mode: SortMode,

    /// Search query
    pub search_query: String,

    /// Multi-select mode enabled
    pub multi_select_mode: bool,

    /// Selected items in multi-select mode
    pub selected_items: Vec<usize>,

    /// Show stats bar
    pub show_stats_bar: bool,

    /// CPU usage history (last 30 data points)
    pub cpu_history: Vec<u64>,

    /// Memory usage history (last 30 data points)
    pub memory_history: Vec<u64>,

    /// VM count history (last 30 data points)
    pub vm_count_history: Vec<u64>,
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
            sort_mode: SortMode::Default,
            search_query: String::new(),
            multi_select_mode: false,
            selected_items: Vec::new(),
            show_stats_bar: true,
            cpu_history: vec![
                45, 52, 48, 55, 60, 58, 62, 65, 63, 68, 70, 67, 72, 75, 73, 78, 80, 77, 75, 72, 70,
                68, 65, 62, 60, 58, 55, 52, 50, 48,
            ],
            memory_history: vec![
                60, 62, 65, 68, 70, 72, 75, 77, 80, 82, 85, 83, 80, 78, 75, 72, 70, 68, 65, 62, 60,
                58, 55, 52, 50, 48, 45, 42, 40, 38,
            ],
            vm_count_history: vec![0; 30], // Will be populated as VMs are added
        }
    }

    /// Refresh VMs from Kubernetes
    pub async fn refresh_vms(&mut self) -> Result<()> {
        use crate::kube::KubeClient;

        let client = KubeClient::new().await?;
        let vm_list = client.list_vms(&self.namespace).await?;

        self.vms = vm_list.into_iter().map(|vm| VmInfo::from_vm(&vm)).collect();

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
        let starting = self
            .vms
            .iter()
            .filter(|vm| vm.status == "Starting" || vm.status == "Pending")
            .count();
        let failed = self
            .vms
            .iter()
            .filter(|vm| vm.status == "Failed" || vm.status == "Error")
            .count();

        VmStats {
            total,
            running,
            stopped,
            starting,
            failed,
        }
    }

    /// Toggle multi-select mode
    pub fn toggle_multi_select(&mut self) {
        self.multi_select_mode = !self.multi_select_mode;
        if !self.multi_select_mode {
            self.selected_items.clear();
        }
    }

    /// Toggle selection of current item
    pub fn toggle_current_selection(&mut self) {
        if self.multi_select_mode {
            if let Some(pos) = self
                .selected_items
                .iter()
                .position(|&i| i == self.selected_index)
            {
                self.selected_items.remove(pos);
            } else {
                self.selected_items.push(self.selected_index);
            }
        }
    }

    /// Select all items
    pub fn select_all(&mut self) {
        if self.multi_select_mode {
            self.selected_items = (0..self.vms.len()).collect();
        }
    }

    /// Deselect all items
    pub fn deselect_all(&mut self) {
        self.selected_items.clear();
    }

    /// Check if item is selected
    pub fn is_selected(&self, index: usize) -> bool {
        self.selected_items.contains(&index)
    }

    /// Cycle sort mode
    pub fn cycle_sort_mode(&mut self) {
        self.sort_mode = self.sort_mode.next();
        self.apply_sort();
    }

    /// Apply current sort mode
    pub fn apply_sort(&mut self) {
        match self.sort_mode {
            SortMode::Default => {}
            SortMode::NameAsc => self.vms.sort_by(|a, b| a.name.cmp(&b.name)),
            SortMode::NameDesc => self.vms.sort_by(|a, b| b.name.cmp(&a.name)),
            SortMode::StatusAsc => self.vms.sort_by(|a, b| a.status.cmp(&b.status)),
            SortMode::StatusDesc => self.vms.sort_by(|a, b| b.status.cmp(&a.status)),
            SortMode::AgeAsc => self.vms.sort_by(|a, b| a.age.cmp(&b.age)),
            SortMode::AgeDesc => self.vms.sort_by(|a, b| b.age.cmp(&a.age)),
        }
    }

    /// Toggle stats bar visibility
    pub fn toggle_stats_bar(&mut self) {
        self.show_stats_bar = !self.show_stats_bar;
    }

    /// Update history data derived from current VM state
    pub fn update_history(&mut self) {
        let stats = self.get_stats();
        let total = stats.total.max(1) as f64;

        // Derive CPU usage estimate from running VM ratio
        let running_ratio = stats.running as f64 / total;
        let cpu_usage = (running_ratio * 75.0).round() as u64; // Running VMs use ~75% capacity

        // Derive memory usage estimate: running VMs consume memory
        let memory_usage = (running_ratio * 70.0).round() as u64;

        if !self.cpu_history.is_empty() {
            self.cpu_history.remove(0);
            self.cpu_history.push(cpu_usage);
        }

        if !self.memory_history.is_empty() {
            self.memory_history.remove(0);
            self.memory_history.push(memory_usage);
        }

        if !self.vm_count_history.is_empty() {
            self.vm_count_history.remove(0);
            self.vm_count_history.push(self.vms.len() as u64);
        }
    }
}

/// VM statistics
#[derive(Debug, Clone)]
pub struct VmStats {
    pub total: usize,
    pub running: usize,
    pub stopped: usize,
    pub starting: usize,
    pub failed: usize,
}

/// Sort mode for VM list
#[derive(Debug, Clone, PartialEq)]
pub enum SortMode {
    Default,
    NameAsc,
    NameDesc,
    StatusAsc,
    StatusDesc,
    AgeAsc,
    AgeDesc,
}

impl SortMode {
    pub fn next(&self) -> Self {
        match self {
            Self::Default => Self::NameAsc,
            Self::NameAsc => Self::NameDesc,
            Self::NameDesc => Self::StatusAsc,
            Self::StatusAsc => Self::StatusDesc,
            Self::StatusDesc => Self::AgeAsc,
            Self::AgeAsc => Self::AgeDesc,
            Self::AgeDesc => Self::Default,
        }
    }

    pub fn display(&self) -> &str {
        match self {
            Self::Default => "Default",
            Self::NameAsc => "Name ↑",
            Self::NameDesc => "Name ↓",
            Self::StatusAsc => "Status ↑",
            Self::StatusDesc => "Status ↓",
            Self::AgeAsc => "Age ↑",
            Self::AgeDesc => "Age ↓",
        }
    }
}
