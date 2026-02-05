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

    // ========== BACKUP & DISASTER RECOVERY ==========

    /// Create a VM backup
    BackupCreate {
        /// VM name
        vm: String,

        /// Backup name (auto-generated if not specified)
        #[arg(short, long)]
        name: Option<String>,

        /// Backup type (full, incremental, differential)
        #[arg(long, default_value = "full")]
        backup_type: String,

        /// Compression type (gzip, zstd, lz4, none)
        #[arg(long, default_value = "gzip")]
        compression: String,

        /// Disable encryption
        #[arg(long)]
        no_encryption: bool,
    },

    /// List backups
    BackupList {
        /// VM name (optional, shows all if not specified)
        vm: Option<String>,

        /// Output format (table, yaml, json)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// Show backup details
    BackupGet {
        /// Backup name
        name: String,

        /// Output format (yaml, json)
        #[arg(short, long, default_value = "yaml")]
        output: String,
    },

    /// Delete a backup
    BackupDelete {
        /// Backup name
        name: String,

        /// Skip confirmation
        #[arg(short, long)]
        yes: bool,
    },

    /// Restore VM from backup
    BackupRestore {
        /// Backup name
        backup: String,

        /// Target VM name (defaults to original)
        #[arg(short, long)]
        target: Option<String>,

        /// Start VM after restore
        #[arg(long)]
        start: bool,
    },

    /// Verify backup integrity
    BackupVerify {
        /// Backup name
        name: String,

        /// Verification type (quick, standard, full)
        #[arg(long, default_value = "standard")]
        verification_type: String,
    },

    /// List backup schedules
    BackupSchedules {
        /// Output format (table, yaml, json)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// Create backup schedule
    BackupScheduleCreate {
        /// Schedule name
        name: String,

        /// Schedule type (hourly, daily, weekly, monthly)
        #[arg(long)]
        schedule: String,

        /// VM selector (all, or specific VM name)
        #[arg(long)]
        vm: Option<String>,
    },

    /// Show disaster recovery plan
    RecoveryPlan {
        /// Plan name
        name: String,

        /// Output format (yaml, json)
        #[arg(short, long, default_value = "yaml")]
        output: String,
    },

    /// Execute disaster recovery
    RecoveryExecute {
        /// Plan name
        plan: String,

        /// Dry run - show what would be done
        #[arg(long)]
        dry_run: bool,
    },

    // ========== SECURITY & COMPLIANCE ==========

    /// Scan VM for security vulnerabilities
    SecurityScan {
        /// VM name
        vm: String,

        /// Scan type (quick, standard, deep, compliance)
        #[arg(long, default_value = "standard")]
        scan_type: String,

        /// Include container scanning
        #[arg(long)]
        containers: bool,

        /// Output format (table, json, yaml)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// Show security assessment for a VM
    SecurityAssess {
        /// VM name
        vm: String,

        /// Output format (yaml, json)
        #[arg(short, long, default_value = "yaml")]
        output: String,
    },

    /// Apply security hardening profile
    SecurityHarden {
        /// VM name
        vm: String,

        /// Hardening profile (cis, stig, pci-dss, nist, custom)
        #[arg(short, long, default_value = "cis")]
        profile: String,

        /// Verify only, don't apply changes
        #[arg(long)]
        verify_only: bool,
    },

    /// List hardening profiles
    SecurityProfiles {
        /// Show detailed information
        #[arg(short, long)]
        details: bool,
    },

    /// Run compliance check
    ComplianceCheck {
        /// VM name
        vm: String,

        /// Compliance framework (pci-dss, hipaa, soc2, iso27001, gdpr, nist, cis)
        #[arg(short, long, default_value = "pci-dss")]
        framework: String,

        /// Output format (table, yaml, json)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// Show compliance report
    ComplianceReport {
        /// VM name
        vm: String,

        /// Report ID (optional, shows latest if not provided)
        #[arg(short, long)]
        report_id: Option<String>,

        /// Output format (yaml, json)
        #[arg(short, long, default_value = "yaml")]
        output: String,
    },

    /// List audit events
    AuditList {
        /// VM name (optional, shows all if not provided)
        vm: Option<String>,

        /// Event type filter
        #[arg(long)]
        event_type: Option<String>,

        /// Severity filter (critical, high, medium, low, info)
        #[arg(long)]
        severity: Option<String>,

        /// Show only security events
        #[arg(long)]
        security_only: bool,

        /// Output format (table, yaml, json)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// Show audit log details
    AuditGet {
        /// Log ID
        log_id: String,

        /// Output format (yaml, json)
        #[arg(short, long, default_value = "yaml")]
        output: String,
    },

    /// Show audit statistics
    AuditStats {
        /// VM name (optional, shows all if not provided)
        vm: Option<String>,

        /// Time period (24h, 7d, 30d, 90d)
        #[arg(short, long, default_value = "7d")]
        period: String,

        /// Output format (table, json, yaml)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    // ========== COST MANAGEMENT & OPTIMIZATION ==========

    /// Show VM cost analysis
    CostAnalyze {
        /// VM name (optional, shows all if not provided)
        vm: Option<String>,

        /// Time period (7d, 30d, 90d, 180d, 365d)
        #[arg(short, long, default_value = "30d")]
        period: String,

        /// Output format (table, yaml, json)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// Show cost summary
    CostSummary {
        /// Namespace filter
        #[arg(short, long)]
        namespace: Option<String>,

        /// Time period (7d, 30d, 90d, 180d, 365d)
        #[arg(short, long, default_value = "30d")]
        period: String,

        /// Group by (namespace, team, project)
        #[arg(long)]
        group_by: Option<String>,

        /// Output format (table, yaml, json)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// Generate cost report
    CostReport {
        /// Report type (daily, weekly, monthly, quarterly, yearly)
        #[arg(short, long, default_value = "monthly")]
        report_type: String,

        /// Export format (json, csv, yaml)
        #[arg(short, long, default_value = "json")]
        format: String,

        /// Output file (defaults to stdout)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Manage budgets
    BudgetList {
        /// Output format (table, yaml, json)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// Create a budget
    BudgetCreate {
        /// Budget name
        name: String,

        /// Budget amount
        #[arg(short, long)]
        amount: f64,

        /// Budget period (daily, weekly, monthly, quarterly, yearly)
        #[arg(short, long, default_value = "monthly")]
        period: String,

        /// Scope (global, namespace:<name>, team:<name>, project:<name>)
        #[arg(short, long, default_value = "global")]
        scope: String,

        /// Alert threshold (e.g., 80 for 80%)
        #[arg(long)]
        alert_threshold: Option<f64>,
    },

    /// Show budget status
    BudgetStatus {
        /// Budget name
        name: String,

        /// Output format (yaml, json)
        #[arg(short, long, default_value = "yaml")]
        output: String,
    },

    /// Get cost optimization recommendations
    CostOptimize {
        /// VM name (optional, shows all if not provided)
        vm: Option<String>,

        /// Show only high priority recommendations
        #[arg(long)]
        high_priority_only: bool,

        /// Output format (table, yaml, json)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// Show cost waste report
    CostWaste {
        /// Waste type filter (idle, oversized, storage, snapshots)
        #[arg(long)]
        waste_type: Option<String>,

        /// Minimum monthly waste to show
        #[arg(long, default_value = "10")]
        min_waste: f64,

        /// Output format (table, yaml, json)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// Forecast costs
    CostForecast {
        /// Budget to compare against
        #[arg(short, long)]
        budget: Option<f64>,

        /// Forecast period (7d, 30d, 90d)
        #[arg(short, long, default_value = "30d")]
        period: String,

        /// Output format (table, yaml, json)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    // ========== AUTOMATION & ORCHESTRATION ==========

    /// List automation rules
    AutomationList {
        /// Show only enabled rules
        #[arg(long)]
        enabled_only: bool,

        /// Output format (table, yaml, json)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// Create automation rule
    AutomationCreate {
        /// Rule name
        name: String,

        /// Description
        #[arg(short, long)]
        description: Option<String>,

        /// Trigger type (manual, schedule, event, metric)
        #[arg(short, long, default_value = "manual")]
        trigger: String,

        /// Enable immediately
        #[arg(long)]
        enable: bool,
    },

    /// Show automation rule details
    AutomationGet {
        /// Rule ID or name
        rule: String,

        /// Output format (yaml, json)
        #[arg(short, long, default_value = "yaml")]
        output: String,
    },

    /// Execute automation rule
    AutomationRun {
        /// Rule ID or name
        rule: String,

        /// Dry run - show what would be done
        #[arg(long)]
        dry_run: bool,
    },

    /// List workflows
    WorkflowList {
        /// Output format (table, yaml, json)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// Create workflow
    WorkflowCreate {
        /// Workflow name
        name: String,

        /// Description
        #[arg(short, long)]
        description: Option<String>,

        /// Template (provisioning, disaster-recovery, maintenance)
        #[arg(short, long)]
        template: Option<String>,
    },

    /// Show workflow details
    WorkflowGet {
        /// Workflow ID or name
        workflow: String,

        /// Output format (yaml, json)
        #[arg(short, long, default_value = "yaml")]
        output: String,
    },

    /// Execute workflow
    WorkflowRun {
        /// Workflow ID or name
        workflow: String,

        /// Show execution progress
        #[arg(short, long)]
        watch: bool,
    },

    /// List workflow executions
    WorkflowExecutions {
        /// Workflow ID or name (optional, shows all if not provided)
        workflow: Option<String>,

        /// Limit results
        #[arg(short, long, default_value = "10")]
        limit: usize,

        /// Output format (table, yaml, json)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// List scheduled tasks
    ScheduleList {
        /// Show only enabled tasks
        #[arg(long)]
        enabled_only: bool,

        /// Output format (table, yaml, json)
        #[arg(short, long, default_value = "table")]
        output: String,
    },

    /// Create scheduled task
    ScheduleCreate {
        /// Task name
        name: String,

        /// Rule ID to execute
        #[arg(short, long)]
        rule: String,

        /// Schedule (hourly, daily, weekly, interval:3600)
        #[arg(short, long)]
        schedule: String,

        /// Enable immediately
        #[arg(long)]
        enable: bool,
    },
}
