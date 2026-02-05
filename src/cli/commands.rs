/// Additional CLI commands for advanced features
use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum AdvancedCommands {
    /// Show detailed VM status with resource usage
    Status {
        /// VM name
        name: String,

        /// Watch mode - continuously update status
        #[arg(short, long)]
        watch: bool,
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

    /// Create multiple VMs from a batch file
    Batch {
        /// Path to batch configuration file
        file: String,

        /// Namespace for all VMs
        #[arg(short, long)]
        namespace: Option<String>,

        /// Dry run
        #[arg(long)]
        dry_run: bool,
    },

    /// Export VM configuration
    Export {
        /// VM name
        name: String,

        /// Output file
        #[arg(short, long)]
        output: Option<String>,

        /// Export as KubeVirt manifest
        #[arg(long)]
        kubevirt: bool,
    },

    /// Interactive VM creation wizard
    #[command(name = "wizard")]
    Wizard {
        /// VM name (optional, will prompt if not provided)
        name: Option<String>,
    },

    /// Inject SSH public key into VM via cloud-init
    SshKey {
        /// VM name (for existing VM) or config file
        target: String,

        /// Path to SSH public key file
        #[arg(short, long)]
        key: String,

        /// Username to add key for
        #[arg(short, long, default_value = "zorvia")]
        user: String,
    },

    /// Show resource usage across VMs
    Resources {
        /// Show all namespaces
        #[arg(short = 'A', long)]
        all_namespaces: bool,

        /// Sort by (cpu, memory, name)
        #[arg(long, default_value = "name")]
        sort_by: String,
    },
}
