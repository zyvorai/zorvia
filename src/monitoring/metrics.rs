// Metrics Collector - Real-time resource usage tracking

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// VM resource metrics at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VMMetrics {
    pub timestamp: DateTime<Utc>,
    pub cpu: CPUMetrics,
    pub memory: MemoryMetrics,
    pub disk: DiskMetrics,
    pub network: NetworkMetrics,
}

impl VMMetrics {
    pub fn new() -> Self {
        Self {
            timestamp: Utc::now(),
            cpu: CPUMetrics::default(),
            memory: MemoryMetrics::default(),
            disk: DiskMetrics::default(),
            network: NetworkMetrics::default(),
        }
    }
}

impl Default for VMMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// CPU metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CPUMetrics {
    pub usage_percent: f64,
    pub cores_allocated: u32,
    pub cores_used: f64,
    pub system_percent: f64,
    pub user_percent: f64,
    pub idle_percent: f64,
}

impl CPUMetrics {
    pub fn usage_description(&self) -> String {
        if self.usage_percent < 50.0 {
            "Low".to_string()
        } else if self.usage_percent < 70.0 {
            "Moderate".to_string()
        } else if self.usage_percent < 90.0 {
            "High".to_string()
        } else {
            "Critical".to_string()
        }
    }
}

/// Memory metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryMetrics {
    pub usage_percent: f64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub total_bytes: u64,
    pub cache_bytes: u64,
    pub swap_used_bytes: u64,
}

impl MemoryMetrics {
    pub fn used_gb(&self) -> f64 {
        self.used_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }

    pub fn total_gb(&self) -> f64 {
        self.total_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }

    pub fn available_gb(&self) -> f64 {
        self.available_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    }
}

/// Disk I/O metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiskMetrics {
    pub read_bytes_per_sec: u64,
    pub write_bytes_per_sec: u64,
    pub read_ops_per_sec: u64,
    pub write_ops_per_sec: u64,
    pub usage_percent: f64,
    pub used_bytes: u64,
    pub total_bytes: u64,
}

impl DiskMetrics {
    pub fn read_mb_per_sec(&self) -> f64 {
        self.read_bytes_per_sec as f64 / (1024.0 * 1024.0)
    }

    pub fn write_mb_per_sec(&self) -> f64 {
        self.write_bytes_per_sec as f64 / (1024.0 * 1024.0)
    }

    pub fn total_iops(&self) -> u64 {
        self.read_ops_per_sec + self.write_ops_per_sec
    }
}

/// Network I/O metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NetworkMetrics {
    pub rx_bytes_per_sec: u64,
    pub tx_bytes_per_sec: u64,
    pub rx_packets_per_sec: u64,
    pub tx_packets_per_sec: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
}

impl NetworkMetrics {
    pub fn rx_mb_per_sec(&self) -> f64 {
        self.rx_bytes_per_sec as f64 / (1024.0 * 1024.0)
    }

    pub fn tx_mb_per_sec(&self) -> f64 {
        self.tx_bytes_per_sec as f64 / (1024.0 * 1024.0)
    }

    pub fn total_bandwidth_mb_per_sec(&self) -> f64 {
        self.rx_mb_per_sec() + self.tx_mb_per_sec()
    }
}

/// Resource usage summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub cpu_percent: f64,
    pub memory_percent: f64,
    pub disk_percent: f64,
    pub network_mb_per_sec: f64,
}

impl ResourceUsage {
    pub fn from_metrics(metrics: &VMMetrics) -> Self {
        Self {
            cpu_percent: metrics.cpu.usage_percent,
            memory_percent: metrics.memory.usage_percent,
            disk_percent: metrics.disk.usage_percent,
            network_mb_per_sec: metrics.network.total_bandwidth_mb_per_sec(),
        }
    }

    pub fn overall_usage(&self) -> f64 {
        (self.cpu_percent + self.memory_percent + self.disk_percent) / 3.0
    }
}

/// Metrics collector for VMs
pub struct MetricsCollector {
    #[allow(dead_code)]
    namespace: String,
}

impl MetricsCollector {
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
        }
    }

    /// Collect current metrics for a VM
    pub async fn collect(&self, vm_name: &str) -> Result<VMMetrics> {
        // In a real implementation, this would query:
        // 1. KubeVirt VMI metrics endpoint
        // 2. Prometheus metrics (if available)
        // 3. VM guest agent metrics

        // For now, return mock metrics
        Ok(self.create_mock_metrics(vm_name))
    }

    /// Collect metrics for multiple VMs
    pub async fn collect_multiple(&self, vm_names: &[String]) -> Result<Vec<(String, VMMetrics)>> {
        let mut results = Vec::new();
        for vm_name in vm_names {
            let metrics = self.collect(vm_name).await?;
            results.push((vm_name.clone(), metrics));
        }
        Ok(results)
    }

    /// Collect historical metrics (simulated)
    pub async fn collect_historical(
        &self,
        vm_name: &str,
        duration_seconds: u64,
    ) -> Result<Vec<VMMetrics>> {
        // In a real implementation, query time-series database
        let points = (duration_seconds / 60).min(100); // One point per minute, max 100
        let mut metrics = Vec::new();

        for i in 0..points {
            let mut m = self.create_mock_metrics(vm_name);
            m.timestamp = Utc::now() - chrono::Duration::seconds((points - i) as i64 * 60);
            metrics.push(m);
        }

        Ok(metrics)
    }

    // Helper to create mock metrics for demonstration
    fn create_mock_metrics(&self, _vm_name: &str) -> VMMetrics {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        VMMetrics {
            timestamp: Utc::now(),
            cpu: CPUMetrics {
                usage_percent: rng.gen_range(30.0..85.0),
                cores_allocated: 4,
                cores_used: rng.gen_range(1.2..3.4),
                system_percent: rng.gen_range(5.0..15.0),
                user_percent: rng.gen_range(20.0..70.0),
                idle_percent: rng.gen_range(15.0..70.0),
            },
            memory: MemoryMetrics {
                usage_percent: rng.gen_range(50.0..80.0),
                used_bytes: rng.gen_range(4_000_000_000..10_000_000_000),
                available_bytes: rng.gen_range(2_000_000_000..6_000_000_000),
                total_bytes: 16_000_000_000,
                cache_bytes: rng.gen_range(500_000_000..2_000_000_000),
                swap_used_bytes: rng.gen_range(0..500_000_000),
            },
            disk: DiskMetrics {
                read_bytes_per_sec: rng.gen_range(5_000_000..50_000_000),
                write_bytes_per_sec: rng.gen_range(2_000_000..20_000_000),
                read_ops_per_sec: rng.gen_range(100..1000),
                write_ops_per_sec: rng.gen_range(50..500),
                usage_percent: rng.gen_range(40.0..75.0),
                used_bytes: rng.gen_range(20_000_000_000..80_000_000_000),
                total_bytes: 100_000_000_000,
            },
            network: NetworkMetrics {
                rx_bytes_per_sec: rng.gen_range(500_000..5_000_000),
                tx_bytes_per_sec: rng.gen_range(1_000_000..8_000_000),
                rx_packets_per_sec: rng.gen_range(500..5000),
                tx_packets_per_sec: rng.gen_range(800..6000),
                rx_errors: 0,
                tx_errors: 0,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_metrics_creation() {
        let metrics = VMMetrics::new();
        assert!(metrics.cpu.usage_percent >= 0.0);
    }

    #[test]
    fn test_cpu_usage_description() {
        let mut cpu = CPUMetrics::default();

        cpu.usage_percent = 40.0;
        assert_eq!(cpu.usage_description(), "Low");

        cpu.usage_percent = 60.0;
        assert_eq!(cpu.usage_description(), "Moderate");

        cpu.usage_percent = 80.0;
        assert_eq!(cpu.usage_description(), "High");

        cpu.usage_percent = 95.0;
        assert_eq!(cpu.usage_description(), "Critical");
    }

    #[test]
    fn test_memory_metrics_gb_conversion() {
        let memory = MemoryMetrics {
            used_bytes: 4_294_967_296,       // 4 GiB
            total_bytes: 17_179_869_184,     // 16 GiB
            available_bytes: 12_884_901_888, // 12 GiB
            ..Default::default()
        };

        assert!((memory.used_gb() - 4.0).abs() < 0.1);
        assert!((memory.total_gb() - 16.0).abs() < 0.1);
        assert!((memory.available_gb() - 12.0).abs() < 0.1);
    }

    #[test]
    fn test_disk_metrics_conversions() {
        let disk = DiskMetrics {
            read_bytes_per_sec: 10_485_760, // 10 MiB/s
            write_bytes_per_sec: 5_242_880, // 5 MiB/s
            read_ops_per_sec: 100,
            write_ops_per_sec: 50,
            ..Default::default()
        };

        assert!((disk.read_mb_per_sec() - 10.0).abs() < 0.1);
        assert!((disk.write_mb_per_sec() - 5.0).abs() < 0.1);
        assert_eq!(disk.total_iops(), 150);
    }

    #[test]
    fn test_network_metrics_conversions() {
        let network = NetworkMetrics {
            rx_bytes_per_sec: 2_097_152, // 2 MiB/s
            tx_bytes_per_sec: 4_194_304, // 4 MiB/s
            ..Default::default()
        };

        assert!((network.rx_mb_per_sec() - 2.0).abs() < 0.1);
        assert!((network.tx_mb_per_sec() - 4.0).abs() < 0.1);
        assert!((network.total_bandwidth_mb_per_sec() - 6.0).abs() < 0.1);
    }

    #[test]
    fn test_resource_usage_from_metrics() {
        let metrics = VMMetrics {
            timestamp: Utc::now(),
            cpu: CPUMetrics {
                usage_percent: 75.0,
                ..Default::default()
            },
            memory: MemoryMetrics {
                usage_percent: 65.0,
                ..Default::default()
            },
            disk: DiskMetrics {
                usage_percent: 55.0,
                ..Default::default()
            },
            network: NetworkMetrics::default(),
        };

        let usage = ResourceUsage::from_metrics(&metrics);
        assert_eq!(usage.cpu_percent, 75.0);
        assert_eq!(usage.memory_percent, 65.0);
        assert_eq!(usage.disk_percent, 55.0);
        assert!((usage.overall_usage() - 65.0).abs() < 0.1);
    }

    #[tokio::test]
    async fn test_metrics_collector() {
        let collector = MetricsCollector::new("default");
        let metrics = collector.collect("test-vm").await.unwrap();

        assert!(metrics.cpu.usage_percent >= 0.0 && metrics.cpu.usage_percent <= 100.0);
        assert!(metrics.memory.usage_percent >= 0.0 && metrics.memory.usage_percent <= 100.0);
    }
}
