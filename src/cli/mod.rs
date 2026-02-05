use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "zorvia")]
#[command(about = "Craft VMs for KubeVirt with Rust power!", long_about = None)]
#[command(version)]
pub struct Cli {
    /// Kubernetes namespace
    #[arg(long, default_value = "default", env = "ZORVIA_NAMESPACE")]
    pub namespace: String,

    /// Path to kubeconfig file
    #[arg(long, env = "KUBECONFIG")]
    pub kubeconfig: Option<String>,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Create a new VM
    Create {
        /// VM name
        name: String,

        /// Use a template (ubuntu, centos, fedora, debian, rhel, windows)
        #[arg(short, long)]
        template: Option<String>,

        /// Load configuration from file
        #[arg(short, long)]
        from_file: Option<String>,

        /// Number of CPU cores
        #[arg(long)]
        cpus: Option<u32>,

        /// Memory size (e.g., 4Gi, 8Gi)
        #[arg(long)]
        memory: Option<String>,

        /// Disk size (e.g., 20Gi, 40Gi)
        #[arg(long)]
        disk_size: Option<String>,

        /// Storage class for disks
        #[arg(long)]
        storage_class: Option<String>,

        /// Container disk image
        #[arg(long)]
        container_disk: Option<String>,

        /// Cloud-init user data file
        #[arg(long)]
        cloud_init: Option<String>,

        /// Dry run (don't create, just show manifest)
        #[arg(long)]
        dry_run: bool,

        /// Output format (yaml, json)
        #[arg(short, long, default_value = "yaml")]
        output: String,
    },

    /// List VMs in the namespace
    List {
        /// Show all namespaces
        #[arg(short = 'A', long)]
        all_namespaces: bool,

        /// Output format (table, yaml, json)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// Get details of a VM
    Get {
        /// VM name
        name: String,

        /// Output format (yaml, json)
        #[arg(short, long, default_value = "yaml")]
        output: String,
    },

    /// Delete a VM
    Delete {
        /// VM name
        name: String,

        /// Skip confirmation
        #[arg(short, long)]
        yes: bool,
    },

    /// Start a VM
    Start {
        /// VM name
        name: String,
    },

    /// Stop a VM
    Stop {
        /// VM name
        name: String,
    },

    /// Restart a VM
    Restart {
        /// VM name
        name: String,
    },

    /// Generate a VM manifest without creating it
    Generate {
        /// VM name
        name: String,

        /// Use a template
        #[arg(short, long)]
        template: Option<String>,

        /// Load configuration from file
        #[arg(short, long)]
        from_file: Option<String>,

        /// Number of CPU cores
        #[arg(long)]
        cpus: Option<u32>,

        /// Memory size
        #[arg(long)]
        memory: Option<String>,

        /// Disk size
        #[arg(long)]
        disk_size: Option<String>,

        /// Output file (defaults to stdout)
        #[arg(short, long)]
        output: Option<String>,

        /// Output format (yaml, json)
        #[arg(long, default_value = "yaml")]
        format: String,

        /// Generate KubeVirt VirtualMachine CRD instead of VMConfig
        #[arg(long)]
        kubevirt: bool,
    },

    /// List available templates
    Templates,

    /// Show template details
    Template {
        /// Template name
        name: String,

        /// Output format (yaml, json)
        #[arg(short, long, default_value = "yaml")]
        output: String,
    },

    /// Validate a VM configuration file
    Validate {
        /// Path to configuration file
        file: String,
    },

    /// Show detailed VM status with resource information
    Status {
        /// VM name
        name: String,

        /// Watch mode - continuously update status
        #[arg(short, long)]
        watch: bool,

        /// Update interval in seconds (for watch mode)
        #[arg(long, default_value = "3")]
        interval: u64,
    },

    /// Clone an existing VM
    Clone {
        /// Source VM name
        source: String,

        /// New VM name
        target: String,

        /// Start the cloned VM immediately
        #[arg(long)]
        start: bool,
    },

    /// Show resource usage summary
    Resources {
        /// Show all namespaces
        #[arg(short = 'A', long)]
        all_namespaces: bool,

        /// Sort by (name, cpu, memory)
        #[arg(long, default_value = "name")]
        sort_by: String,
    },

    /// Export VM configuration
    Export {
        /// VM name
        name: String,

        /// Output file (defaults to stdout)
        #[arg(short, long)]
        output: Option<String>,

        /// Export as KubeVirt manifest
        #[arg(long)]
        kubevirt: bool,
    },

    /// Interactive VM creation wizard
    Wizard {
        /// VM name (optional, will prompt if not provided)
        name: Option<String>,
    },

    /// Create multiple VMs from a batch configuration file
    Batch {
        /// Path to batch configuration file (YAML/JSON)
        file: String,

        /// Namespace override for all VMs
        #[arg(short, long)]
        namespace: Option<String>,

        /// Dry run - show what would be created
        #[arg(long)]
        dry_run: bool,

        /// Continue on errors instead of stopping
        #[arg(long)]
        continue_on_error: bool,
    },

    // ========== INNOVATIVE FEATURES ==========

    /// List VM resource profiles (dev, prod, high-perf, etc.)
    Profiles {
        /// Show detailed information
        #[arg(short, long)]
        details: bool,
    },

    /// Show specific profile details
    Profile {
        /// Profile name
        name: String,

        /// Output format (yaml, json)
        #[arg(short, long, default_value = "yaml")]
        output: String,
    },

    /// List multi-VM blueprints (LAMP, Kubernetes, 3-tier, etc.)
    Blueprints {
        /// Filter by tag
        #[arg(short, long)]
        tag: Option<String>,

        /// Show detailed information
        #[arg(short, long)]
        details: bool,
    },

    /// Show specific blueprint details
    Blueprint {
        /// Blueprint name
        name: String,

        /// Output format (yaml, json)
        #[arg(short, long, default_value = "yaml")]
        output: String,
    },

    /// Deploy a multi-VM blueprint
    Deploy {
        /// Blueprint name
        blueprint: String,

        /// Name prefix for VMs (default: blueprint name)
        #[arg(short, long)]
        prefix: Option<String>,

        /// Start VMs after creation
        #[arg(long)]
        start: bool,

        /// Dry run - show what would be created
        #[arg(long)]
        dry_run: bool,
    },

    /// Run health check on a VM configuration or running VM
    Health {
        /// VM name (for running VM) or config file path
        target: String,

        /// Show detailed checks
        #[arg(short, long)]
        detailed: bool,
    },

    /// Get resource recommendations for a workload
    Recommend {
        /// Workload type (web, database, cache, ci, etc.)
        workload: String,

        /// Show alternative profiles
        #[arg(short, long)]
        alternatives: bool,
    },

    // ========== VM SNAPSHOTS & BACKUP ==========

    /// Create a VM snapshot
    SnapshotCreate {
        /// VM name
        vm: String,

        /// Snapshot name (optional, auto-generated if not provided)
        #[arg(short, long)]
        name: Option<String>,

        /// Description of the snapshot
        #[arg(short, long)]
        description: Option<String>,
    },

    /// List snapshots
    SnapshotList {
        /// VM name (optional, shows all snapshots if not provided)
        vm: Option<String>,

        /// Show all namespaces
        #[arg(short = 'A', long)]
        all_namespaces: bool,

        /// Output format (table, yaml, json)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// Show snapshot details
    SnapshotGet {
        /// Snapshot name
        name: String,

        /// Output format (yaml, json)
        #[arg(short, long, default_value = "yaml")]
        output: String,
    },

    /// Delete a snapshot
    SnapshotDelete {
        /// Snapshot name
        name: String,

        /// Skip confirmation
        #[arg(short, long)]
        yes: bool,
    },

    /// Restore VM from snapshot
    SnapshotRestore {
        /// Snapshot name
        snapshot: String,

        /// Target VM name (if different from original)
        #[arg(short, long)]
        target: Option<String>,

        /// Restore in-place (overwrite existing VM)
        #[arg(long)]
        in_place: bool,

        /// Start VM after restore
        #[arg(long)]
        start: bool,
    },

    // ========== PERFORMANCE MONITORING ==========

    /// Show live performance monitoring for a VM
    MonitorLive {
        /// VM name
        vm: String,

        /// Update interval in seconds
        #[arg(short, long, default_value = "5")]
        interval: u64,
    },

    /// Get performance statistics for a VM
    MonitorStats {
        /// VM name
        vm: String,

        /// Time period (5m, 15m, 1h, 6h, 24h, 7d)
        #[arg(short, long, default_value = "1h")]
        period: String,

        /// Output format (table, json, yaml, summary)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// Compare performance of multiple VMs
    MonitorCompare {
        /// VM names to compare
        vms: Vec<String>,

        /// Output format (table, json, yaml)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// Show top VMs by resource usage
    MonitorTop {
        /// Show all namespaces
        #[arg(short = 'A', long)]
        all_namespaces: bool,

        /// Sort by (cpu, memory, disk, score)
        #[arg(long, default_value = "score")]
        sort_by: String,

        /// Number of VMs to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    // ========== DISK MANAGEMENT ==========

    /// Expand VM disk size
    DiskExpand {
        /// VM name
        vm: String,

        /// Disk name
        disk: String,

        /// New size (e.g., 100Gi)
        size: String,

        /// PVC name (if different from disk name)
        #[arg(long)]
        pvc: Option<String>,

        /// Show expansion plan without executing
        #[arg(long)]
        plan: bool,
    },

    /// Check disk health for a VM
    DiskHealth {
        /// VM name
        vm: String,

        /// Show detailed disk information
        #[arg(short, long)]
        detailed: bool,
    },

    /// Generate filesystem expansion script
    DiskScript {
        /// Filesystem type (ext4, xfs, lvm, lvm-xfs, btrfs)
        #[arg(short, long, default_value = "lvm")]
        filesystem: String,

        /// Device path (e.g., /dev/vda)
        #[arg(short, long, default_value = "/dev/vda")]
        device: String,

        /// Output file (defaults to stdout)
        #[arg(short, long)]
        output: Option<String>,

        /// Generate dry-run script
        #[arg(long)]
        dry_run: bool,
    },

    /// Get disk usage statistics for VMs
    DiskUsage {
        /// VM name (optional, shows all VMs if not provided)
        vm: Option<String>,

        /// Sort by (name, usage, size, available)
        #[arg(long, default_value = "usage")]
        sort_by: String,

        /// Output format (table, json, yaml)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    // ========== NETWORK MANAGEMENT ==========

    /// List network interfaces for a VM
    NetworkList {
        /// VM name
        vm: String,

        /// Output format (table, yaml, json)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// Show network interface details
    NetworkGet {
        /// VM name
        vm: String,

        /// Interface name
        interface: String,

        /// Output format (yaml, json)
        #[arg(short, long, default_value = "yaml")]
        output: String,
    },

    /// Monitor network bandwidth for a VM
    NetworkBandwidth {
        /// VM name
        vm: String,

        /// Interface name (optional, shows all if not provided)
        #[arg(short, long)]
        interface: Option<String>,

        /// Watch mode - continuously update
        #[arg(short, long)]
        watch: bool,

        /// Update interval in seconds (for watch mode)
        #[arg(long, default_value = "5")]
        interval: u64,
    },

    /// Show network traffic analysis
    NetworkTraffic {
        /// VM name
        vm: String,

        /// Interface name
        #[arg(short, long)]
        interface: Option<String>,

        /// Time period (5m, 15m, 1h, 6h)
        #[arg(short, long, default_value = "15m")]
        period: String,

        /// Show top N talkers
        #[arg(long, default_value = "10")]
        top: usize,

        /// Output format (table, json, yaml)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// List network policies
    NetworkPolicies {
        /// Show all namespaces
        #[arg(short = 'A', long)]
        all_namespaces: bool,

        /// Output format (table, yaml, json)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// Show network policy details
    NetworkPolicy {
        /// Policy name
        name: String,

        /// Output format (yaml, json)
        #[arg(short, long, default_value = "yaml")]
        output: String,
    },

    // ========== VM MIGRATION & HIGH AVAILABILITY ==========

    /// Migrate a VM to another node
    Migrate {
        /// VM name
        vm: String,

        /// Target node (auto-select if not specified)
        #[arg(short, long)]
        target_node: Option<String>,

        /// Migration type (live, offline, post-copy)
        #[arg(long, default_value = "live")]
        migration_type: String,

        /// Show migration plan without executing
        #[arg(long)]
        plan: bool,
    },

    /// Show migration status
    MigrationStatus {
        /// VM name
        vm: String,

        /// Watch mode - continuously update status
        #[arg(short, long)]
        watch: bool,

        /// Update interval in seconds (for watch mode)
        #[arg(long, default_value = "5")]
        interval: u64,
    },

    /// List migrations
    MigrationList {
        /// Show all namespaces
        #[arg(short = 'A', long)]
        all_namespaces: bool,

        /// Filter by state (running, succeeded, failed)
        #[arg(long)]
        state: Option<String>,

        /// Output format (table, yaml, json)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// Configure VM high availability
    HAConfig {
        /// VM name
        vm: String,

        /// Enable HA
        #[arg(long)]
        enable: bool,

        /// Disable HA
        #[arg(long)]
        disable: bool,

        /// HA priority (critical, high, normal, low)
        #[arg(long)]
        priority: Option<String>,

        /// Eviction strategy (live-migrate, shutdown, none)
        #[arg(long)]
        eviction_strategy: Option<String>,
    },

    /// Show VM HA status
    HAStatus {
        /// VM name
        vm: String,

        /// Output format (yaml, json)
        #[arg(short, long, default_value = "yaml")]
        output: String,
    },

    /// Evacuate/drain a node
    EvacuateNode {
        /// Node name
        node: String,

        /// Reason for evacuation
        #[arg(short, long)]
        reason: Option<String>,

        /// Max parallel migrations
        #[arg(long, default_value = "2")]
        max_parallel: u32,

        /// Timeout in seconds
        #[arg(long, default_value = "3600")]
        timeout: u64,

        /// Force evacuation
        #[arg(long)]
        force: bool,

        /// Show evacuation plan without executing
        #[arg(long)]
        plan: bool,
    },

    /// Show node evacuation status
    EvacuationStatus {
        /// Node name
        node: String,

        /// Watch mode - continuously update status
        #[arg(short, long)]
        watch: bool,
    },
}
