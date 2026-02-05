// VM Profiles System - Pre-configured resource profiles for different workloads
// This is an innovative feature that makes VM creation easier and optimized

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use once_cell::sync::Lazy;

pub static PROFILES: Lazy<ProfileManager> = Lazy::new(ProfileManager::new);

/// Profile represents a pre-configured resource allocation template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub description: String,
    pub cpu_cores: u32,
    pub cpu_sockets: u32,
    pub cpu_threads: u32,
    pub memory: String,
    pub disk_size: String,
    pub use_cases: Vec<String>,
    pub recommended_os: Vec<String>,
}

/// ProfileManager manages all available profiles
pub struct ProfileManager {
    profiles: HashMap<String, Profile>,
}

impl ProfileManager {
    pub fn new() -> Self {
        let mut profiles = HashMap::new();

        // Development Profile - Minimal resources
        profiles.insert("dev".to_string(), Profile {
            name: "dev".to_string(),
            description: "Development environment - minimal resources for testing".to_string(),
            cpu_cores: 1,
            cpu_sockets: 1,
            cpu_threads: 1,
            memory: "2Gi".to_string(),
            disk_size: "10Gi".to_string(),
            use_cases: vec![
                "Local development".to_string(),
                "Testing".to_string(),
                "Learning".to_string(),
            ],
            recommended_os: vec![
                "ubuntu".to_string(),
                "alpine".to_string(),
                "debian".to_string(),
            ],
        });

        // Testing Profile - Moderate resources
        profiles.insert("test".to_string(), Profile {
            name: "test".to_string(),
            description: "Testing environment - moderate resources for CI/CD".to_string(),
            cpu_cores: 2,
            cpu_sockets: 1,
            cpu_threads: 1,
            memory: "4Gi".to_string(),
            disk_size: "20Gi".to_string(),
            use_cases: vec![
                "CI/CD pipelines".to_string(),
                "Integration testing".to_string(),
                "QA environments".to_string(),
            ],
            recommended_os: vec![
                "ubuntu".to_string(),
                "almalinux".to_string(),
                "rocky".to_string(),
            ],
        });

        // Production Profile - Balanced resources
        profiles.insert("prod".to_string(), Profile {
            name: "prod".to_string(),
            description: "Production environment - balanced resources for reliability".to_string(),
            cpu_cores: 4,
            cpu_sockets: 1,
            cpu_threads: 1,
            memory: "8Gi".to_string(),
            disk_size: "40Gi".to_string(),
            use_cases: vec![
                "Production workloads".to_string(),
                "Web applications".to_string(),
                "API servers".to_string(),
            ],
            recommended_os: vec![
                "ubuntu-22.04".to_string(),
                "almalinux".to_string(),
                "rocky".to_string(),
                "debian".to_string(),
            ],
        });

        // High Performance Profile
        profiles.insert("high-perf".to_string(), Profile {
            name: "high-perf".to_string(),
            description: "High performance - maximum resources for demanding workloads".to_string(),
            cpu_cores: 8,
            cpu_sockets: 2,
            cpu_threads: 1,
            memory: "16Gi".to_string(),
            disk_size: "100Gi".to_string(),
            use_cases: vec![
                "Database servers".to_string(),
                "Data processing".to_string(),
                "Machine learning".to_string(),
                "High traffic applications".to_string(),
            ],
            recommended_os: vec![
                "ubuntu-24.04".to_string(),
                "almalinux".to_string(),
                "rocky".to_string(),
            ],
        });

        // Micro Service Profile - Container optimized
        profiles.insert("microservice".to_string(), Profile {
            name: "microservice".to_string(),
            description: "Microservice - optimized for containerized applications".to_string(),
            cpu_cores: 2,
            cpu_sockets: 1,
            cpu_threads: 1,
            memory: "4Gi".to_string(),
            disk_size: "20Gi".to_string(),
            use_cases: vec![
                "Container runtime".to_string(),
                "Kubernetes nodes".to_string(),
                "Docker hosts".to_string(),
                "Microservices".to_string(),
            ],
            recommended_os: vec![
                "flatcar".to_string(),
                "alpine".to_string(),
                "ubuntu".to_string(),
            ],
        });

        // Database Profile - I/O optimized
        profiles.insert("database".to_string(), Profile {
            name: "database".to_string(),
            description: "Database - optimized for I/O intensive workloads".to_string(),
            cpu_cores: 6,
            cpu_sockets: 1,
            cpu_threads: 1,
            memory: "16Gi".to_string(),
            disk_size: "200Gi".to_string(),
            use_cases: vec![
                "Database servers".to_string(),
                "PostgreSQL".to_string(),
                "MySQL".to_string(),
                "MongoDB".to_string(),
                "Redis".to_string(),
            ],
            recommended_os: vec![
                "ubuntu-22.04".to_string(),
                "debian-12".to_string(),
                "almalinux".to_string(),
            ],
        });

        // Web Server Profile
        profiles.insert("web".to_string(), Profile {
            name: "web".to_string(),
            description: "Web server - optimized for HTTP workloads".to_string(),
            cpu_cores: 4,
            cpu_sockets: 1,
            cpu_threads: 1,
            memory: "8Gi".to_string(),
            disk_size: "40Gi".to_string(),
            use_cases: vec![
                "Web applications".to_string(),
                "Nginx".to_string(),
                "Apache".to_string(),
                "Static sites".to_string(),
                "Reverse proxy".to_string(),
            ],
            recommended_os: vec![
                "ubuntu".to_string(),
                "alpine".to_string(),
                "debian".to_string(),
            ],
        });

        // Minimal Profile - Ultra lightweight
        profiles.insert("minimal".to_string(), Profile {
            name: "minimal".to_string(),
            description: "Minimal - ultra lightweight for basic tasks".to_string(),
            cpu_cores: 1,
            cpu_sockets: 1,
            cpu_threads: 1,
            memory: "512Mi".to_string(),
            disk_size: "5Gi".to_string(),
            use_cases: vec![
                "DNS server".to_string(),
                "Jump host".to_string(),
                "Monitoring agent".to_string(),
                "Log collector".to_string(),
            ],
            recommended_os: vec![
                "alpine".to_string(),
            ],
        });

        Self { profiles }
    }

    pub fn get(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    pub fn list(&self) -> Vec<&Profile> {
        let mut profiles: Vec<_> = self.profiles.values().collect();
        profiles.sort_by(|a, b| a.name.cmp(&b.name));
        profiles
    }

    pub fn exists(&self, name: &str) -> bool {
        self.profiles.contains_key(name)
    }

    /// Get profile recommendation based on use case keywords
    pub fn recommend(&self, use_case: &str) -> Vec<&Profile> {
        let use_case_lower = use_case.to_lowercase();
        let mut matches: Vec<_> = self.profiles
            .values()
            .filter(|p| {
                p.use_cases.iter().any(|uc|
                    uc.to_lowercase().contains(&use_case_lower) ||
                    use_case_lower.contains(&uc.to_lowercase())
                )
            })
            .collect();

        matches.sort_by(|a, b| a.name.cmp(&b.name));
        matches
    }
}

impl Default for ProfileManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_manager() {
        let manager = ProfileManager::new();

        assert!(manager.exists("dev"));
        assert!(manager.exists("prod"));
        assert!(manager.exists("high-perf"));
        assert!(!manager.exists("nonexistent"));

        let dev = manager.get("dev").unwrap();
        assert_eq!(dev.cpu_cores, 1);
        assert_eq!(dev.memory, "2Gi");
    }

    #[test]
    fn test_list_profiles() {
        let manager = ProfileManager::new();
        let profiles = manager.list();

        assert!(profiles.len() >= 8);
    }

    #[test]
    fn test_recommend() {
        let manager = ProfileManager::new();

        let db_profiles = manager.recommend("database");
        assert!(!db_profiles.is_empty());
        assert!(db_profiles.iter().any(|p| p.name == "database"));

        let web_profiles = manager.recommend("web");
        assert!(!web_profiles.is_empty());
        assert!(web_profiles.iter().any(|p| p.name == "web"));
    }
}
