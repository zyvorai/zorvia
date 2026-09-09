//! Prometheus text-format parser for virt-launcher / kubevirt_vmi_* samples.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromSample {
    pub name: String,
    pub labels: BTreeMap<String, String>,
    pub value: f64,
}

/// Parse a subset of Prometheus text exposition (gauge/counter lines).
pub fn parse_prom_text(text: &str) -> Vec<PromSample> {
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (name_labels, value_part) = match line.rsplit_once(char::is_whitespace) {
            Some(pair) => pair,
            None => continue,
        };
        let Ok(value) = value_part.trim().parse::<f64>() else {
            continue;
        };
        let (name, labels) = split_name_labels(name_labels.trim());
        if name.starts_with("kubevirt_vmi_") || name.starts_with("zorvia_") {
            out.push(PromSample {
                name,
                labels,
                value,
            });
        }
    }
    out
}

fn split_name_labels(spec: &str) -> (String, BTreeMap<String, String>) {
    if let Some(idx) = spec.find('{') {
        let name = spec[..idx].to_string();
        let inner = spec[idx + 1..].trim_end_matches('}');
        let mut labels = BTreeMap::new();
        for part in inner.split(',') {
            if let Some((k, v)) = part.split_once('=') {
                labels.insert(k.trim().to_string(), v.trim().trim_matches('"').to_string());
            }
        }
        (name, labels)
    } else {
        (spec.to_string(), BTreeMap::new())
    }
}

/// Reduce virt-launcher samples into the Fabric metrics shape.
pub fn fabric_from_prom(samples: &[PromSample], vm: &str) -> (f64, u64, u64) {
    let for_vm = |s: &PromSample| {
        s.labels
            .get("name")
            .or_else(|| s.labels.get("vm"))
            .map(|n| n == vm)
            .unwrap_or(true)
    };
    let cpu = samples
        .iter()
        .filter(|s| s.name == "kubevirt_vmi_vcpu_seconds" && for_vm(s))
        .map(|s| s.value)
        .sum::<f64>();
    let mem = samples
        .iter()
        .find(|s| {
            (s.name == "kubevirt_vmi_memory_available_bytes"
                || s.name == "kubevirt_vmi_memory_usable_bytes")
                && for_vm(s)
        })
        .map(|s| s.value as u64)
        .unwrap_or(0);
    let disk = samples
        .iter()
        .find(|s| s.name.contains("storage") && for_vm(s))
        .map(|s| s.value as u64)
        .unwrap_or(0);
    (cpu, mem, disk)
}

pub fn servicemonitor_yaml(namespace: &str) -> String {
    format!(
        r#"apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: kubevirt-vmis
  namespace: {namespace}
  labels:
    app.kubernetes.io/managed-by: zorvia
spec:
  selector:
    matchLabels:
      kubevirt.io: virt-launcher
  namespaceSelector:
    any: true
  endpoints:
    - port: metrics
      interval: 30s
      honorLabels: true
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_kubevirt_samples() {
        let text = r#"
# HELP kubevirt_vmi_vcpu_seconds VCPU time
kubevirt_vmi_vcpu_seconds{name="web-01",namespace="default",id="0"} 12.5
kubevirt_vmi_memory_available_bytes{name="web-01",namespace="default"} 2147483648
ignored_metric 1
"#;
        let samples = parse_prom_text(text);
        assert_eq!(samples.len(), 2);
        let (cpu, mem, _) = fabric_from_prom(&samples, "web-01");
        assert_eq!(cpu, 12.5);
        assert_eq!(mem, 2147483648);
    }

    #[test]
    fn servicemonitor_targets_virt_launcher() {
        let y = servicemonitor_yaml("monitoring");
        assert!(y.contains("virt-launcher"));
        assert!(y.contains("namespace: monitoring"));
    }
}
