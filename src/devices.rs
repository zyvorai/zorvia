//! Passthrough devices (GPUs, SR-IOV virtual functions, other host devices): what
//! the cluster offers, and whether a VM that asks for some can actually start.
//!
//! KubeVirt only schedules a device that (1) a device plugin advertises as an
//! extended resource on at least one node with capacity left, and (2) is listed in
//! the KubeVirt CR's `permittedHostDevices`. Without a check, a VM that misses either
//! is created and then sits Pending or is refused with an obscure condition. This
//! module is pure (the API handlers gather the inputs) and unit-tested.

use crate::config::HostDeviceConfig;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// Pseudo-devices KubeVirt itself advertises on every node; not passthrough hardware.
pub fn is_builtin(resource: &str) -> bool {
    resource.starts_with("devices.kubevirt.io/")
}

/// `4`, `1k` (=1000, KubeVirt's builtin counts), anything else is not a count.
fn parse_count(q: &str) -> Option<u64> {
    let q = q.trim();
    match q.strip_suffix('k') {
        Some(n) => n.parse::<u64>().ok().map(|n| n * 1000),
        None => q.parse().ok(),
    }
}

/// The device-plugin resources in a node's allocatable map: extended resources
/// (`vendor.com/name`) with a usable count, minus kubernetes.io/* and hugepages.
pub fn extended_resources(allocatable: &BTreeMap<String, String>) -> BTreeMap<String, u64> {
    allocatable
        .iter()
        .filter(|(k, _)| {
            k.contains('/') && !k.starts_with("kubernetes.io/") && !k.starts_with("hugepages-")
        })
        .filter_map(|(k, v)| parse_count(v).map(|n| (k.clone(), n)))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NodeDevices {
    pub node: String,
    /// Passthrough candidates: extended resources that are not KubeVirt builtins.
    pub devices: BTreeMap<String, u64>,
    /// KubeVirt's own pseudo-devices (kvm, tun, ...), listed for completeness.
    pub builtin: BTreeMap<String, u64>,
}

impl NodeDevices {
    pub fn from_allocatable(node: &str, allocatable: &BTreeMap<String, String>) -> Self {
        let (builtin, devices) = extended_resources(allocatable)
            .into_iter()
            .partition(|(k, _)| is_builtin(k));
        Self {
            node: node.to_string(),
            devices,
            builtin,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PermittedDevice {
    pub resource_name: String,
    /// `pci`, `mediated` or `usb`.
    pub kind: &'static str,
    /// The selector KubeVirt matches (PCI `vendor:product`, mdev type name, or USB selectors).
    pub selector: String,
    /// A device plugin outside KubeVirt manages the resource.
    pub external: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct KubevirtDevices {
    pub feature_gates: Vec<String>,
    pub permitted: Vec<PermittedDevice>,
}

impl KubevirtDevices {
    /// From the KubeVirt CR's `spec.configuration`.
    pub fn from_configuration(cfg: &Value) -> Self {
        let gates = cfg
            .pointer("/developerConfiguration/featureGates")
            .and_then(|g| g.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let mut permitted = Vec::new();
        let list = |key: &str| {
            cfg.pointer(&format!("/permittedHostDevices/{key}"))
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default()
        };
        let text = |v: &Value, k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        for d in list("pciHostDevices") {
            permitted.push(PermittedDevice {
                resource_name: text(&d, "resourceName"),
                kind: "pci",
                selector: text(&d, "pciVendorSelector"),
                external: d
                    .get("externalResourceProvider")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false),
            });
        }
        for d in list("mediatedDevices") {
            permitted.push(PermittedDevice {
                resource_name: text(&d, "resourceName"),
                kind: "mediated",
                selector: text(&d, "mdevNameSelector"),
                external: d
                    .get("externalResourceProvider")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false),
            });
        }
        for d in list("usb") {
            let sel = d
                .get("selectors")
                .and_then(|s| s.as_array())
                .map(|a| {
                    a.iter()
                        .map(|s| format!("{}:{}", text(s, "vendor"), text(s, "product")))
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .unwrap_or_default();
            permitted.push(PermittedDevice {
                resource_name: text(&d, "resourceName"),
                kind: "usb",
                selector: sel,
                external: d
                    .get("externalResourceProvider")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false),
            });
        }
        permitted.retain(|p| !p.resource_name.is_empty());
        Self {
            feature_gates: gates,
            permitted,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DeviceInventory {
    pub nodes: Vec<NodeDevices>,
    /// `None` when the KubeVirt CR could not be read (RBAC, or not found).
    pub kubevirt: Option<KubevirtDevices>,
    /// The NetworkAttachmentDefinition CRD (Multus) exists, which SR-IOV needs.
    pub multus_installed: bool,
}

impl DeviceInventory {
    /// Total of `resource` across schedulable nodes, and how many nodes have any.
    fn capacity(&self, resource: &str) -> (u64, usize) {
        let counts: Vec<u64> = self
            .nodes
            .iter()
            .filter_map(|n| n.devices.get(resource).copied())
            .collect();
        (
            counts.iter().sum(),
            counts.iter().filter(|c| **c > 0).count(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Issue {
    /// `error` blocks the VM; `warning` is advice.
    pub severity: &'static str,
    pub subject: String,
    pub message: String,
}

fn err(subject: &str, message: String) -> Issue {
    Issue {
        severity: "error",
        subject: subject.to_string(),
        message,
    }
}

fn warn(subject: &str, message: String) -> Issue {
    Issue {
        severity: "warning",
        subject: subject.to_string(),
        message,
    }
}

/// An SR-IOV network a VM wants, resolved by the caller from the cluster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SriovNetwork {
    pub name: String,
    pub exists: bool,
    /// The NAD's `k8s.v1.cni.cncf.io/resourceName` annotation (the VF pool).
    pub resource_name: Option<String>,
}

pub fn preflight(
    devices: &[HostDeviceConfig],
    sriov: &[SriovNetwork],
    inv: &DeviceInventory,
) -> Vec<Issue> {
    let mut out = Vec::new();
    for d in devices {
        let subject = d.name.as_str();
        if let Some(kv) = &inv.kubevirt {
            if !kv
                .permitted
                .iter()
                .any(|p| p.resource_name == d.device_name)
            {
                out.push(err(
                    subject,
                    format!(
                        "'{}' is not in the KubeVirt CR's spec.configuration.permittedHostDevices, so KubeVirt will refuse to attach it",
                        d.device_name
                    ),
                ));
            }
        }
        match inv.capacity(&d.device_name) {
            (_, 0) if inv.nodes.iter().any(|n| n.devices.contains_key(&d.device_name)) => {
                out.push(err(
                    subject,
                    format!("every '{}' is already in use (allocatable is 0 on every node)", d.device_name),
                ))
            }
            (_, 0) => out.push(err(
                subject,
                format!(
                    "no node advertises '{}': is the device plugin running and the hardware present?",
                    d.device_name
                ),
            )),
            _ => {}
        }
    }
    for n in sriov {
        if !inv.multus_installed {
            out.push(err(
                &n.name,
                "the NetworkAttachmentDefinition CRD is not installed (Multus), which SR-IOV interfaces need".into(),
            ));
            continue;
        }
        if !n.exists {
            out.push(err(
                &n.name,
                format!(
                    "NetworkAttachmentDefinition '{}' does not exist in the VM's namespace",
                    n.name
                ),
            ));
            continue;
        }
        match &n.resource_name {
            None => out.push(warn(
                &n.name,
                "the network has no k8s.v1.cni.cncf.io/resourceName annotation, so the virtual-function pool cannot be checked".into(),
            )),
            Some(r) => {
                if inv.capacity(r).1 == 0 {
                    out.push(err(
                        &n.name,
                        format!("no node has free '{r}' virtual functions (device plugin missing, or all in use)"),
                    ));
                }
            }
        }
    }
    if !devices.is_empty() || !sriov.is_empty() {
        out.push(Issue {
            severity: "warning",
            subject: "live-migration".into(),
            message: "a VM with a passthrough device or SR-IOV interface is pinned to its node: it cannot be live-migrated, and a node drain will stop it".into(),
        });
    }
    out
}

pub fn has_errors(issues: &[Issue]) -> bool {
    issues.iter().any(|i| i.severity == "error")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HostDeviceKind;
    use serde_json::json;

    fn alloc(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn dev(name: &str, resource: &str) -> HostDeviceConfig {
        HostDeviceConfig {
            name: name.into(),
            device_name: resource.into(),
            kind: HostDeviceKind::Gpu,
        }
    }

    fn inventory(gpus: u64, permitted: bool) -> DeviceInventory {
        let a = alloc(&[
            ("cpu", "12"),
            ("memory", "32Gi"),
            ("hugepages-2Mi", "0"),
            ("nvidia.com/GA102GL_A10", &gpus.to_string()),
            ("intel.com/sriov_vfs", "0"),
            ("devices.kubevirt.io/kvm", "1k"),
            ("kubernetes.io/foo", "3"),
        ]);
        DeviceInventory {
            nodes: vec![NodeDevices::from_allocatable("n1", &a)],
            kubevirt: Some(KubevirtDevices {
                feature_gates: vec![],
                permitted: if permitted {
                    vec![PermittedDevice {
                        resource_name: "nvidia.com/GA102GL_A10".into(),
                        kind: "pci",
                        selector: "10DE:2236".into(),
                        external: false,
                    }]
                } else {
                    vec![]
                },
            }),
            multus_installed: true,
        }
    }

    #[test]
    fn allocatable_is_split_into_passthrough_candidates_and_builtins() {
        let n = NodeDevices::from_allocatable(
            "n1",
            &alloc(&[
                ("cpu", "4"),
                ("nvidia.com/GA102GL_A10", "2"),
                ("devices.kubevirt.io/kvm", "1k"),
                ("hugepages-1Gi", "0"),
                ("kubernetes.io/x", "1"),
                ("vendor.com/odd", "1.5"),
            ]),
        );
        assert_eq!(n.devices.get("nvidia.com/GA102GL_A10"), Some(&2));
        assert_eq!(
            n.devices.len(),
            1,
            "no cpu, hugepages, kubernetes.io or non-integer: {:?}",
            n.devices
        );
        assert_eq!(n.builtin.get("devices.kubevirt.io/kvm"), Some(&1000));
    }

    #[test]
    fn kubevirt_configuration_yields_permitted_devices_of_every_kind() {
        let cfg = json!({
            "developerConfiguration": {"featureGates": ["GPU", "Snapshot"]},
            "permittedHostDevices": {
                "pciHostDevices": [{"resourceName": "nvidia.com/A10", "pciVendorSelector": "10DE:2236"}],
                "mediatedDevices": [{"resourceName": "nvidia.com/vgpu", "mdevNameSelector": "NVIDIA A10-4Q", "externalResourceProvider": true}],
                "usb": [{"resourceName": "kubevirt.io/key", "selectors": [{"vendor": "046d", "product": "c52b"}]}]
            }
        });
        let k = KubevirtDevices::from_configuration(&cfg);
        assert_eq!(k.feature_gates, vec!["GPU", "Snapshot"]);
        let kinds: Vec<_> = k
            .permitted
            .iter()
            .map(|p| (p.kind, p.resource_name.as_str()))
            .collect();
        assert_eq!(
            kinds,
            vec![
                ("pci", "nvidia.com/A10"),
                ("mediated", "nvidia.com/vgpu"),
                ("usb", "kubevirt.io/key")
            ]
        );
        assert!(k.permitted[1].external);
        assert_eq!(k.permitted[2].selector, "046d:c52b");
        assert!(KubevirtDevices::from_configuration(&json!({}))
            .permitted
            .is_empty());
    }

    #[test]
    fn a_permitted_available_device_passes_with_only_the_migration_warning() {
        let issues = preflight(
            &[dev("gpu0", "nvidia.com/GA102GL_A10")],
            &[],
            &inventory(2, true),
        );
        assert!(!has_errors(&issues), "{issues:?}");
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].subject, "live-migration");
    }

    #[test]
    fn missing_permission_missing_hardware_and_exhausted_devices_are_errors() {
        // Advertised and free, but KubeVirt does not permit it.
        let i = preflight(
            &[dev("gpu0", "nvidia.com/GA102GL_A10")],
            &[],
            &inventory(2, false),
        );
        assert!(i
            .iter()
            .any(|x| x.severity == "error" && x.message.contains("permittedHostDevices")));
        // Permitted, but every unit is taken.
        let i = preflight(
            &[dev("gpu0", "nvidia.com/GA102GL_A10")],
            &[],
            &inventory(0, true),
        );
        assert!(i.iter().any(|x| x.message.contains("already in use")));
        // Nothing advertises that resource at all.
        let i = preflight(&[dev("x", "amd.com/mi300")], &[], &inventory(2, true));
        assert!(i.iter().any(|x| x.message.contains("no node advertises")));
        // KubeVirt config unreadable: do not invent a permission error.
        let mut inv = inventory(2, true);
        inv.kubevirt = None;
        assert!(!has_errors(&preflight(
            &[dev("gpu0", "nvidia.com/GA102GL_A10")],
            &[],
            &inv
        )));
    }

    #[test]
    fn sriov_needs_multus_an_existing_network_and_free_virtual_functions() {
        let net = |exists, res: Option<&str>| SriovNetwork {
            name: "vf-net".into(),
            exists,
            resource_name: res.map(str::to_string),
        };
        let inv = inventory(1, true);
        // Free VFs: fine (warning only).
        let mut ok = inv.clone();
        ok.nodes[0].devices.insert("intel.com/sriov_vfs".into(), 8);
        assert!(!has_errors(&preflight(
            &[],
            &[net(true, Some("intel.com/sriov_vfs"))],
            &ok
        )));
        // Pool exists but is empty.
        assert!(
            preflight(&[], &[net(true, Some("intel.com/sriov_vfs"))], &inv)
                .iter()
                .any(|i| i.message.contains("no node has free"))
        );
        // Network missing, annotation missing, Multus missing.
        assert!(preflight(&[], &[net(false, None)], &inv)
            .iter()
            .any(|i| i.message.contains("does not exist")));
        let w = preflight(&[], &[net(true, None)], &inv);
        assert!(!has_errors(&w) && w.iter().any(|i| i.message.contains("resourceName")));
        let mut no_multus = inv.clone();
        no_multus.multus_installed = false;
        assert!(preflight(&[], &[net(true, None)], &no_multus)
            .iter()
            .any(|i| i.message.contains("Multus")));
    }

    #[test]
    fn no_devices_means_no_issues_at_all() {
        assert!(preflight(&[], &[], &inventory(1, true)).is_empty());
    }
}
