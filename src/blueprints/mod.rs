// Multi-VM Blueprints - Deploy complete application stacks with one command
// This is an innovative feature for deploying complex topologies

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use once_cell::sync::Lazy;

pub static BLUEPRINTS: Lazy<BlueprintManager> = Lazy::new(BlueprintManager::new);

/// VMSpec defines a single VM in a blueprint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VMSpec {
    pub name: String,
    pub template: String,
    pub profile: Option<String>,
    pub cpu: Option<u32>,
    pub memory: Option<String>,
    pub disk_size: Option<String>,
    pub depends_on: Vec<String>,
    pub labels: HashMap<String, String>,
}

/// Blueprint defines a multi-VM deployment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blueprint {
    pub name: String,
    pub description: String,
    pub vms: Vec<VMSpec>,
    pub tags: Vec<String>,
}

/// BlueprintManager manages pre-defined blueprints
pub struct BlueprintManager {
    blueprints: HashMap<String, Blueprint>,
}

impl BlueprintManager {
    pub fn new() -> Self {
        let mut blueprints = HashMap::new();

        // LAMP Stack Blueprint
        blueprints.insert("lamp".to_string(), Blueprint {
            name: "lamp".to_string(),
            description: "LAMP Stack (Linux + Apache + MySQL + PHP)".to_string(),
            vms: vec![
                VMSpec {
                    name: "mysql-db".to_string(),
                    template: "ubuntu".to_string(),
                    profile: Some("database".to_string()),
                    cpu: None,
                    memory: None,
                    disk_size: Some("100Gi".to_string()),
                    depends_on: vec![],
                    labels: HashMap::from([
                        ("tier".to_string(), "database".to_string()),
                        ("app".to_string(), "lamp".to_string()),
                    ]),
                },
                VMSpec {
                    name: "web-server".to_string(),
                    template: "ubuntu".to_string(),
                    profile: Some("web".to_string()),
                    cpu: None,
                    memory: None,
                    disk_size: None,
                    depends_on: vec!["mysql-db".to_string()],
                    labels: HashMap::from([
                        ("tier".to_string(), "frontend".to_string()),
                        ("app".to_string(), "lamp".to_string()),
                    ]),
                },
            ],
            tags: vec!["web".to_string(), "database".to_string(), "classic".to_string()],
        });

        // Kubernetes Cluster Blueprint
        blueprints.insert("k8s-cluster".to_string(), Blueprint {
            name: "k8s-cluster".to_string(),
            description: "Kubernetes cluster (1 control plane + 2 workers)".to_string(),
            vms: vec![
                VMSpec {
                    name: "k8s-control".to_string(),
                    template: "ubuntu-22.04".to_string(),
                    profile: Some("prod".to_string()),
                    cpu: Some(4),
                    memory: Some("8Gi".to_string()),
                    disk_size: Some("50Gi".to_string()),
                    depends_on: vec![],
                    labels: HashMap::from([
                        ("role".to_string(), "control-plane".to_string()),
                        ("cluster".to_string(), "k8s".to_string()),
                    ]),
                },
                VMSpec {
                    name: "k8s-worker-1".to_string(),
                    template: "ubuntu-22.04".to_string(),
                    profile: Some("prod".to_string()),
                    cpu: Some(4),
                    memory: Some("16Gi".to_string()),
                    disk_size: Some("100Gi".to_string()),
                    depends_on: vec!["k8s-control".to_string()],
                    labels: HashMap::from([
                        ("role".to_string(), "worker".to_string()),
                        ("cluster".to_string(), "k8s".to_string()),
                    ]),
                },
                VMSpec {
                    name: "k8s-worker-2".to_string(),
                    template: "ubuntu-22.04".to_string(),
                    profile: Some("prod".to_string()),
                    cpu: Some(4),
                    memory: Some("16Gi".to_string()),
                    disk_size: Some("100Gi".to_string()),
                    depends_on: vec!["k8s-control".to_string()],
                    labels: HashMap::from([
                        ("role".to_string(), "worker".to_string()),
                        ("cluster".to_string(), "k8s".to_string()),
                    ]),
                },
            ],
            tags: vec!["kubernetes".to_string(), "cluster".to_string(), "container".to_string()],
        });

        // 3-Tier Web App Blueprint
        blueprints.insert("3tier".to_string(), Blueprint {
            name: "3tier".to_string(),
            description: "3-tier web application (Web + App + Database)".to_string(),
            vms: vec![
                VMSpec {
                    name: "postgres-db".to_string(),
                    template: "almalinux".to_string(),
                    profile: Some("database".to_string()),
                    cpu: None,
                    memory: Some("16Gi".to_string()),
                    disk_size: Some("200Gi".to_string()),
                    depends_on: vec![],
                    labels: HashMap::from([
                        ("tier".to_string(), "database".to_string()),
                        ("app".to_string(), "3tier".to_string()),
                    ]),
                },
                VMSpec {
                    name: "app-server".to_string(),
                    template: "ubuntu".to_string(),
                    profile: Some("prod".to_string()),
                    cpu: Some(4),
                    memory: Some("8Gi".to_string()),
                    disk_size: Some("40Gi".to_string()),
                    depends_on: vec!["postgres-db".to_string()],
                    labels: HashMap::from([
                        ("tier".to_string(), "application".to_string()),
                        ("app".to_string(), "3tier".to_string()),
                    ]),
                },
                VMSpec {
                    name: "nginx-lb".to_string(),
                    template: "alpine".to_string(),
                    profile: Some("web".to_string()),
                    cpu: Some(2),
                    memory: Some("4Gi".to_string()),
                    disk_size: Some("20Gi".to_string()),
                    depends_on: vec!["app-server".to_string()],
                    labels: HashMap::from([
                        ("tier".to_string(), "frontend".to_string()),
                        ("app".to_string(), "3tier".to_string()),
                    ]),
                },
            ],
            tags: vec!["web".to_string(), "microservices".to_string(), "production".to_string()],
        });

        // Development Stack Blueprint
        blueprints.insert("dev-stack".to_string(), Blueprint {
            name: "dev-stack".to_string(),
            description: "Development stack (Code server + DB + Redis)".to_string(),
            vms: vec![
                VMSpec {
                    name: "dev-db".to_string(),
                    template: "ubuntu".to_string(),
                    profile: Some("test".to_string()),
                    cpu: Some(2),
                    memory: Some("4Gi".to_string()),
                    disk_size: Some("20Gi".to_string()),
                    depends_on: vec![],
                    labels: HashMap::from([
                        ("env".to_string(), "development".to_string()),
                        ("type".to_string(), "database".to_string()),
                    ]),
                },
                VMSpec {
                    name: "dev-cache".to_string(),
                    template: "alpine".to_string(),
                    profile: Some("minimal".to_string()),
                    cpu: Some(1),
                    memory: Some("2Gi".to_string()),
                    disk_size: Some("10Gi".to_string()),
                    depends_on: vec![],
                    labels: HashMap::from([
                        ("env".to_string(), "development".to_string()),
                        ("type".to_string(), "cache".to_string()),
                    ]),
                },
                VMSpec {
                    name: "dev-workspace".to_string(),
                    template: "ubuntu".to_string(),
                    profile: Some("dev".to_string()),
                    cpu: Some(4),
                    memory: Some("8Gi".to_string()),
                    disk_size: Some("40Gi".to_string()),
                    depends_on: vec!["dev-db".to_string(), "dev-cache".to_string()],
                    labels: HashMap::from([
                        ("env".to_string(), "development".to_string()),
                        ("type".to_string(), "workspace".to_string()),
                    ]),
                },
            ],
            tags: vec!["development".to_string(), "testing".to_string()],
        });

        // CI/CD Pipeline Blueprint
        blueprints.insert("cicd".to_string(), Blueprint {
            name: "cicd".to_string(),
            description: "CI/CD Pipeline (Jenkins + GitLab + Artifact Registry)".to_string(),
            vms: vec![
                VMSpec {
                    name: "gitlab-server".to_string(),
                    template: "ubuntu-22.04".to_string(),
                    profile: Some("prod".to_string()),
                    cpu: Some(4),
                    memory: Some("16Gi".to_string()),
                    disk_size: Some("100Gi".to_string()),
                    depends_on: vec![],
                    labels: HashMap::from([
                        ("component".to_string(), "scm".to_string()),
                        ("pipeline".to_string(), "cicd".to_string()),
                    ]),
                },
                VMSpec {
                    name: "jenkins-server".to_string(),
                    template: "ubuntu-22.04".to_string(),
                    profile: Some("prod".to_string()),
                    cpu: Some(4),
                    memory: Some("8Gi".to_string()),
                    disk_size: Some("80Gi".to_string()),
                    depends_on: vec!["gitlab-server".to_string()],
                    labels: HashMap::from([
                        ("component".to_string(), "ci".to_string()),
                        ("pipeline".to_string(), "cicd".to_string()),
                    ]),
                },
                VMSpec {
                    name: "artifact-registry".to_string(),
                    template: "almalinux".to_string(),
                    profile: Some("prod".to_string()),
                    cpu: Some(2),
                    memory: Some("8Gi".to_string()),
                    disk_size: Some("200Gi".to_string()),
                    depends_on: vec![],
                    labels: HashMap::from([
                        ("component".to_string(), "registry".to_string()),
                        ("pipeline".to_string(), "cicd".to_string()),
                    ]),
                },
            ],
            tags: vec!["cicd".to_string(), "automation".to_string(), "devops".to_string()],
        });

        Self { blueprints }
    }

    pub fn get(&self, name: &str) -> Option<&Blueprint> {
        self.blueprints.get(name)
    }

    pub fn list(&self) -> Vec<&Blueprint> {
        let mut blueprints: Vec<_> = self.blueprints.values().collect();
        blueprints.sort_by(|a, b| a.name.cmp(&b.name));
        blueprints
    }

    pub fn exists(&self, name: &str) -> bool {
        self.blueprints.contains_key(name)
    }

    /// Search blueprints by tag
    pub fn search_by_tag(&self, tag: &str) -> Vec<&Blueprint> {
        let tag_lower = tag.to_lowercase();
        let mut matches: Vec<_> = self.blueprints
            .values()
            .filter(|b| b.tags.iter().any(|t| t.to_lowercase().contains(&tag_lower)))
            .collect();

        matches.sort_by(|a, b| a.name.cmp(&b.name));
        matches
    }
}

impl Default for BlueprintManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blueprint_manager() {
        let manager = BlueprintManager::new();

        assert!(manager.exists("lamp"));
        assert!(manager.exists("k8s-cluster"));
        assert!(manager.exists("3tier"));
        assert!(!manager.exists("nonexistent"));

        let lamp = manager.get("lamp").unwrap();
        assert_eq!(lamp.vms.len(), 2);
    }

    #[test]
    fn test_list_blueprints() {
        let manager = BlueprintManager::new();
        let blueprints = manager.list();

        assert!(blueprints.len() >= 5);
    }

    #[test]
    fn test_search_by_tag() {
        let manager = BlueprintManager::new();

        let web_blueprints = manager.search_by_tag("web");
        assert!(!web_blueprints.is_empty());

        let k8s_blueprints = manager.search_by_tag("kubernetes");
        assert!(!k8s_blueprints.is_empty());
    }

    #[test]
    fn test_vm_dependencies() {
        let manager = BlueprintManager::new();
        let lamp = manager.get("lamp").unwrap();

        let web_vm = lamp.vms.iter().find(|v| v.name == "web-server").unwrap();
        assert!(web_vm.depends_on.contains(&"mysql-db".to_string()));
    }
}
