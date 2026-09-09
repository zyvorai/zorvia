//! Production lifecycle helpers that do not require a live cluster to test.
//!
//! Path builders and validation for KubeVirt VMI subresources used by
//! pause/resume, serial console, and VNC.

use anyhow::{bail, Result};
use regex::Regex;
use std::sync::OnceLock;

/// RFC 1123 DNS label used by Kubernetes object names.
fn k8s_name_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[a-z0-9]([-a-z0-9]*[a-z0-9])?$").expect("valid regex"))
}

/// Validate a Kubernetes namespace or object name.
pub fn validate_k8s_name(kind: &str, name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 63 {
        bail!("{kind} must be 1-63 characters");
    }
    if !k8s_name_re().is_match(name) {
        bail!("{kind} '{name}' is not a valid RFC 1123 DNS label");
    }
    Ok(())
}

/// Allowed VMI subresources Zorvia proxies or invokes.
pub const VMI_SUBRESOURCES: &[&str] = &[
    "console",
    "vnc",
    "pause",
    "unpause",
    "freeze",
    "unfreeze",
    "guestosinfo",
    "userlist",
    "filesystemlist",
    "addvolume",
    "removevolume",
];

/// WebSocket subprotocol expected by KubeVirt for a given console type.
pub fn kubevirt_ws_subprotocol(subresource: &str) -> Option<&'static str> {
    match subresource {
        "vnc" => Some("binary.kubevirt.io"),
        "console" => Some("plain.kubevirt.io"),
        _ => None,
    }
}

/// Build the apiserver path for a VMI subresource.
pub fn vmi_subresource_path(namespace: &str, name: &str, subresource: &str) -> Result<String> {
    validate_k8s_name("namespace", namespace)?;
    validate_k8s_name("name", name)?;
    if !VMI_SUBRESOURCES.contains(&subresource) {
        bail!("unsupported VMI subresource '{subresource}'");
    }
    Ok(format!(
        "/apis/subresources.kubevirt.io/v1/namespaces/{namespace}/virtualmachineinstances/{name}/{subresource}"
    ))
}

/// Authenticated WebSocket path served by Zorvia for serial or VNC.
pub fn zorvia_console_ws_path(kind: &str, vm: &str) -> Result<String> {
    validate_k8s_name("name", vm)?;
    match kind {
        "console" | "vnc" => Ok(format!("/ws/{kind}/{vm}")),
        other => bail!("unsupported console kind '{other}'"),
    }
}

/// True when KubeVirt printable status / phase indicates a paused guest.
pub fn status_is_paused(printable: Option<&str>, phase: Option<&str>, conditions: &[(&str, &str)]) -> bool {
    if printable
        .map(|s| s.eq_ignore_ascii_case("paused") || s.to_ascii_lowercase().contains("paus"))
        .unwrap_or(false)
    {
        return true;
    }
    if phase
        .map(|s| s.eq_ignore_ascii_case("paused"))
        .unwrap_or(false)
    {
        return true;
    }
    conditions
        .iter()
        .any(|(ty, status)| ty.eq_ignore_ascii_case("Paused") && status.eq_ignore_ascii_case("True"))
}

/// Map a KubeVirt / HTTP error into a client-safe pause/resume message.
pub fn classify_lifecycle_error(action: &str, raw: &str) -> (u16, &'static str, String) {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("notfound") || lower.contains("not found") {
        return (
            404,
            "NOT_FOUND",
            format!("VM instance not found; start the VM before {action}"),
        );
    }
    if lower.contains("forbidden") || lower.contains("unauthorized") {
        return (
            403,
            "FORBIDDEN",
            format!("Not allowed to {action} this VM (check subresources.kubevirt.io RBAC)"),
        );
    }
    if lower.contains("conflict") || lower.contains("already paused") || lower.contains("not paused") {
        return (409, "CONFLICT", format!("Cannot {action}: {raw}"));
    }
    (
        500,
        "LIFECYCLE_FAILED",
        format!("Failed to {action} VM"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_and_invalid_names() {
        assert!(validate_k8s_name("name", "").is_err());
        assert!(validate_k8s_name("name", "VM_1").is_err());
        assert!(validate_k8s_name("name", "has.dot").is_err());
        assert!(validate_k8s_name("name", "prod-db").is_ok());
        assert!(validate_k8s_name("name", "a").is_ok());
    }

    #[test]
    fn builds_pause_unpause_paths() {
        let pause = vmi_subresource_path("default", "prod-db", "pause").unwrap();
        assert_eq!(
            pause,
            "/apis/subresources.kubevirt.io/v1/namespaces/default/virtualmachineinstances/prod-db/pause"
        );
        let unpause = vmi_subresource_path("kubevirt", "web-01", "unpause").unwrap();
        assert!(unpause.ends_with("/web-01/unpause"));
    }

    #[test]
    fn rejects_unknown_subresource() {
        assert!(vmi_subresource_path("default", "vm", "explode").is_err());
    }

    #[test]
    fn vnc_and_console_protocols() {
        assert_eq!(kubevirt_ws_subprotocol("vnc"), Some("binary.kubevirt.io"));
        assert_eq!(kubevirt_ws_subprotocol("console"), Some("plain.kubevirt.io"));
        assert_eq!(kubevirt_ws_subprotocol("pause"), None);
        assert_eq!(zorvia_console_ws_path("vnc", "win-01").unwrap(), "/ws/vnc/win-01");
        assert_eq!(
            zorvia_console_ws_path("console", "linux-01").unwrap(),
            "/ws/console/linux-01"
        );
        assert!(zorvia_console_ws_path("spice", "vm").is_err());
    }

    #[test]
    fn paused_detection() {
        assert!(status_is_paused(Some("Paused"), None, &[]));
        assert!(status_is_paused(None, Some("Paused"), &[]));
        assert!(status_is_paused(None, Some("Running"), &[("Paused", "True")]));
        assert!(!status_is_paused(Some("Running"), Some("Running"), &[("Ready", "True")]));
    }

    #[test]
    fn classifies_not_found() {
        let (code, kind, _) = classify_lifecycle_error("pause", "ApiError: NotFound");
        assert_eq!(code, 404);
        assert_eq!(kind, "NOT_FOUND");
    }
}
