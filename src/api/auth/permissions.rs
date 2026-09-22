//! HTTP API permissions for route-level authorization.
//!
//! These are distinct from the CLI multitenancy `Permission` enum: they map
//! the three JWT roles (Admin / User / Viewer) onto coarse route gates.

use super::jwt::Role;

/// Coarse permissions enforced by the HTTP API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApiPermission {
    VmRead,
    VmPower,
    VmCreate,
    VmDelete,
    StorageAdmin,
    ClusterAdmin,
    UsersAdmin,
}

impl ApiPermission {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VmRead => "vm.read",
            Self::VmPower => "vm.power",
            Self::VmCreate => "vm.create",
            Self::VmDelete => "vm.delete",
            Self::StorageAdmin => "storage.admin",
            Self::ClusterAdmin => "cluster.admin",
            Self::UsersAdmin => "users.admin",
        }
    }

    /// Parse a scope string (e.g. from an API token). Unknown values are ignored by callers.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "vm.read" => Some(Self::VmRead),
            "vm.power" => Some(Self::VmPower),
            "vm.create" => Some(Self::VmCreate),
            "vm.delete" => Some(Self::VmDelete),
            "storage.admin" => Some(Self::StorageAdmin),
            "cluster.admin" => Some(Self::ClusterAdmin),
            "users.admin" => Some(Self::UsersAdmin),
            "admin" | "*" | "all" => None, // handled as full role grant
            _ => None,
        }
    }
}

/// Fixed permission sets for JWT roles.
pub fn permissions_for_role(role: &Role) -> Vec<ApiPermission> {
    match role {
        Role::Viewer => vec![ApiPermission::VmRead],
        Role::User => vec![
            ApiPermission::VmRead,
            ApiPermission::VmPower,
            ApiPermission::VmCreate,
        ],
        Role::Admin => vec![
            ApiPermission::VmRead,
            ApiPermission::VmPower,
            ApiPermission::VmCreate,
            ApiPermission::VmDelete,
            ApiPermission::StorageAdmin,
            ApiPermission::ClusterAdmin,
            ApiPermission::UsersAdmin,
        ],
    }
}

pub fn role_has_permission(role: &Role, required: ApiPermission) -> bool {
    permissions_for_role(role).contains(&required)
}

/// Resolve permissions for an API token: explicit scopes intersected with the
/// token's role ceiling. Empty scopes → full role set.
pub fn permissions_for_token(role: &Role, scopes: &[String]) -> Vec<ApiPermission> {
    let ceiling = permissions_for_role(role);
    if scopes.is_empty()
        || scopes
            .iter()
            .any(|s| matches!(s.as_str(), "admin" | "*" | "all"))
    {
        return ceiling;
    }
    let mut out = Vec::new();
    for s in scopes {
        if let Some(p) = ApiPermission::parse(s) {
            if ceiling.contains(&p) && !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// Map HTTP method + API path (under `/api`, e.g. `/vms/foo/start`) to a
/// required permission. `None` means any authenticated identity may proceed
/// (typically GET/read). Public paths are handled before this is consulted.
pub fn required_permission(method: &str, path: &str) -> Option<ApiPermission> {
    let method = method.to_ascii_uppercase();
    let path = path.trim_end_matches('/');

    // Auth self-service (any authenticated user)
    if path.starts_with("/v1/auth/") {
        return None;
    }

    // Audit JSONL export is admin-only even on GET
    if path == "/audit/export"
        || path.ends_with("/audit/export")
        || path == "/audit/logs/export"
        || path.ends_with("/audit/logs/export")
    {
        return Some(ApiPermission::ClusterAdmin);
    }

    // User administration
    if path.starts_with("/v1/users") {
        return Some(ApiPermission::UsersAdmin);
    }

    // Scoped API token management
    if path.starts_with("/v1/api-tokens") {
        return Some(ApiPermission::UsersAdmin);
    }

    if method == "GET" || method == "HEAD" {
        return None;
    }

    // Power actions
    if path.ends_with("/start")
        || path.ends_with("/stop")
        || path.ends_with("/restart")
        || path.ends_with("/pause")
        || path.ends_with("/resume")
    {
        return Some(ApiPermission::VmPower);
    }

    // Delete VM
    if method == "DELETE"
        && (path.starts_with("/vms/") && path.matches('/').count() == 2
            || path.starts_with("/v1/vms/") && path.matches('/').count() == 4)
    {
        return Some(ApiPermission::VmDelete);
    }

    // Storage / Rook administration
    if path.starts_with("/storage/") {
        return Some(ApiPermission::StorageAdmin);
    }

    // Cluster-admin surface: policies, schedulers, webhooks, alerts mutate, quotas, warm pools
    if path.starts_with("/webhooks")
        || path.starts_with("/schedules/")
        || path.starts_with("/alerts")
        || path.starts_with("/warm-pools")
        || path.starts_with("/network-policies")
        || path.starts_with("/backups/policies")
        || path.starts_with("/v1/quotas")
        || path.starts_with("/system/compliance")
        || path.starts_with("/v1/enterprise/")
    {
        return Some(ApiPermission::ClusterAdmin);
    }

    // Migration cancel / create
    if path.contains("/migrate") || path.contains("/migrations") {
        return Some(ApiPermission::VmCreate);
    }

    // Snapshot delete / revert
    if path.contains("/snapshots") {
        if method == "DELETE" || path.ends_with("/revert") {
            return Some(ApiPermission::VmCreate);
        }
        return Some(ApiPermission::VmCreate);
    }

    // Backup mutate
    if path.starts_with("/backups") {
        return Some(ApiPermission::VmCreate);
    }

    // Template deploy, image download, VM create/update/hotplug/clone/etc.
    if path == "/vms"
        || path.starts_with("/vms/")
        || path.starts_with("/v1/vms/")
        || path.starts_with("/v1/snapshots")
        || path.starts_with("/images/")
        || path.starts_with("/templates/")
        || path.starts_with("/datavolumes/")
        || path.starts_with("/v1/kryton/")
    {
        return Some(ApiPermission::VmCreate);
    }

    // Default deny for unknown mutating methods → cluster.admin
    Some(ApiPermission::ClusterAdmin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_cannot_write() {
        assert!(role_has_permission(&Role::Viewer, ApiPermission::VmRead));
        assert!(!role_has_permission(&Role::Viewer, ApiPermission::VmPower));
        assert!(!role_has_permission(&Role::Viewer, ApiPermission::VmCreate));
        assert!(!role_has_permission(&Role::Viewer, ApiPermission::VmDelete));
    }

    #[test]
    fn user_can_create_not_delete() {
        assert!(role_has_permission(&Role::User, ApiPermission::VmCreate));
        assert!(role_has_permission(&Role::User, ApiPermission::VmPower));
        assert!(!role_has_permission(&Role::User, ApiPermission::VmDelete));
        assert!(!role_has_permission(
            &Role::User,
            ApiPermission::StorageAdmin
        ));
    }

    #[test]
    fn admin_has_all() {
        assert!(role_has_permission(&Role::Admin, ApiPermission::UsersAdmin));
        assert!(role_has_permission(
            &Role::Admin,
            ApiPermission::ClusterAdmin
        ));
    }

    #[test]
    fn audit_export_requires_cluster_admin() {
        assert_eq!(
            required_permission("GET", "/audit/export"),
            Some(ApiPermission::ClusterAdmin)
        );
        assert_eq!(required_permission("GET", "/audit/logs"), None);
    }

    #[test]
    fn token_scopes_narrow_role() {
        let perms = permissions_for_token(&Role::Admin, &["vm.read".into(), "vm.power".into()]);
        assert_eq!(perms, vec![ApiPermission::VmRead, ApiPermission::VmPower]);
    }

    #[test]
    fn route_map_viewer_mutations() {
        assert_eq!(
            required_permission("POST", "/vms"),
            Some(ApiPermission::VmCreate)
        );
        assert_eq!(
            required_permission("DELETE", "/vms/foo"),
            Some(ApiPermission::VmDelete)
        );
        assert_eq!(
            required_permission("POST", "/vms/foo/start"),
            Some(ApiPermission::VmPower)
        );
        assert_eq!(
            required_permission("POST", "/storage/rook/bootstrap"),
            Some(ApiPermission::StorageAdmin)
        );
        assert_eq!(required_permission("GET", "/vms"), None);
    }
}
