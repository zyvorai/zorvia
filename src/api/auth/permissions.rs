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

    // Commercial administration (contract changes, org membership, quoting,
    // offline import) is admin-only for every method. The rest of
    // /v1/commercial/ is org-scoped inside the handlers, so any authenticated
    // role may reach it (VmRead is held by all roles) and mutating calls
    // don't fall through to the cluster.admin default.
    if path.starts_with("/v1/commercial/admin") {
        return Some(ApiPermission::UsersAdmin);
    }
    if path.starts_with("/v1/commercial/") {
        return if method == "GET" || method == "HEAD" {
            None
        } else {
            Some(ApiPermission::VmRead)
        };
    }

    // Support: exporting or uploading a diagnostic bundle reads cluster
    // state, so it needs cluster.admin for every method. Owner assignment is
    // the support desk (users.admin). Everything else is org-scoped in the
    // store and open to any authenticated role.
    if path.starts_with("/v1/support/") && path.contains("/diagnostics") {
        return Some(ApiPermission::ClusterAdmin);
    }
    if path.starts_with("/v1/support/admin") {
        return Some(ApiPermission::UsersAdmin);
    }
    if path.starts_with("/v1/support/") {
        return if method == "GET" || method == "HEAD" {
            None
        } else {
            Some(ApiPermission::VmRead)
        };
    }

    // Service engagements and managed-operations records: the service desk
    // (users.admin) owns /admin/ routes; the rest is org-scoped in the store
    // and open to any authenticated role.
    if path.starts_with("/v1/services/admin") || path.starts_with("/v1/managed/admin") {
        return Some(ApiPermission::UsersAdmin);
    }
    if path.starts_with("/v1/services/") || path.starts_with("/v1/managed/") {
        return if method == "GET" || method == "HEAD" {
            None
        } else {
            Some(ApiPermission::VmRead)
        };
    }

    // Billing records. The local capacity collector reads cluster state, so
    // it needs cluster.admin; /admin/ routes are the service desk
    // (users.admin); the rest is org-scoped in the store.
    if path.starts_with("/v1/billing/capacity/observe") {
        return Some(ApiPermission::ClusterAdmin);
    }
    if path.starts_with("/v1/billing/admin") {
        return Some(ApiPermission::UsersAdmin);
    }
    if path.starts_with("/v1/billing/") {
        return if method == "GET" || method == "HEAD" {
            None
        } else {
            Some(ApiPermission::VmRead)
        };
    }

    // Cluster-wide pod inventory, logs and exec are admin-only even on GET
    if path.starts_with("/v1/pods") || path.starts_with("/v1/namespaces") {
        return Some(ApiPermission::ClusterAdmin);
    }

    // Rescue mode creates a privileged Kubernetes Job that mounts a VM's
    // own PVC and runs arbitrary offline guest-disk mutations -- exec-
    // equivalent sensitivity, admin-only even on GET (job-status polling
    // still reveals whether/what a rescue op did).
    if path.starts_with("/vms/") && path.contains("/rescue") {
        return Some(ApiPermission::ClusterAdmin);
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

    // Atlas DR failover/promote/demote can flip which cluster is primary --
    // require cluster.admin (the strictest tier) as defense-in-depth beyond
    // Atlas's own token-role check, checked before the general storage.admin
    // rule below. See docs/ATLAS_INTEGRATION.md's Disaster recovery section.
    if path.starts_with("/v1/atlas/dr/")
        && (path.ends_with("/promote") || path.ends_with("/demote") || path.ends_with("/failover"))
    {
        return Some(ApiPermission::ClusterAdmin);
    }

    // Storage / Rook administration, plus Atlas's one write route (volume create) --
    // Atlas's GET routes fall through to the no-permission-required GET/HEAD branch
    // above, same as Kryton's.
    if path.starts_with("/storage/") || path.starts_with("/v1/atlas/") {
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
    fn commercial_admin_routes_need_users_admin() {
        assert_eq!(
            required_permission("GET", "/v1/commercial/admin/orgs"),
            Some(ApiPermission::UsersAdmin)
        );
        assert_eq!(
            required_permission("POST", "/v1/commercial/admin/contracts/x/transition"),
            Some(ApiPermission::UsersAdmin)
        );
        assert_eq!(
            required_permission("POST", "/v1/commercial/quote-requests"),
            Some(ApiPermission::VmRead)
        );
        assert_eq!(required_permission("GET", "/v1/commercial/contracts"), None);
    }

    #[test]
    fn support_routes_are_gated() {
        assert_eq!(
            required_permission("POST", "/v1/support/diagnostics"),
            Some(ApiPermission::ClusterAdmin)
        );
        assert_eq!(
            required_permission("POST", "/v1/support/cases/c1/diagnostics/upload"),
            Some(ApiPermission::ClusterAdmin)
        );
        assert_eq!(
            required_permission("POST", "/v1/support/admin/cases/c1/assign"),
            Some(ApiPermission::UsersAdmin)
        );
        assert_eq!(
            required_permission("POST", "/v1/support/cases"),
            Some(ApiPermission::VmRead)
        );
        assert_eq!(required_permission("GET", "/v1/support/cases"), None);
    }

    #[test]
    fn services_and_managed_routes_are_gated() {
        assert_eq!(
            required_permission("POST", "/v1/services/admin/engagements"),
            Some(ApiPermission::UsersAdmin)
        );
        assert_eq!(
            required_permission("POST", "/v1/managed/admin/enrollments/e1/tasks"),
            Some(ApiPermission::UsersAdmin)
        );
        assert_eq!(
            required_permission("POST", "/v1/managed/enrollments"),
            Some(ApiPermission::VmRead)
        );
        assert_eq!(required_permission("GET", "/v1/services/engagements"), None);
    }

    #[test]
    fn billing_routes_are_gated() {
        assert_eq!(
            required_permission("POST", "/v1/billing/capacity/observe"),
            Some(ApiPermission::ClusterAdmin)
        );
        assert_eq!(
            required_permission("POST", "/v1/billing/admin/invoices/i1/mark-paid"),
            Some(ApiPermission::UsersAdmin)
        );
        assert_eq!(
            required_permission("GET", "/v1/billing/admin/capacity/observations"),
            Some(ApiPermission::UsersAdmin)
        );
        assert_eq!(required_permission("GET", "/v1/billing/invoices"), None);
    }

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

    #[test]
    fn atlas_volume_create_requires_storage_admin_but_reads_are_open() {
        assert_eq!(
            required_permission("POST", "/v1/atlas/volumes"),
            Some(ApiPermission::StorageAdmin)
        );
        assert_eq!(required_permission("GET", "/v1/atlas/volumes"), None);
        assert_eq!(required_permission("GET", "/v1/atlas/status"), None);
        assert_eq!(required_permission("GET", "/v1/atlas/backends"), None);
    }

    #[test]
    fn atlas_dr_failover_promote_demote_require_cluster_admin_not_just_storage_admin() {
        assert_eq!(
            required_permission("POST", "/v1/atlas/dr/mirrors/drm_1/promote"),
            Some(ApiPermission::ClusterAdmin)
        );
        assert_eq!(
            required_permission("POST", "/v1/atlas/dr/mirrors/drm_1/demote"),
            Some(ApiPermission::ClusterAdmin)
        );
        assert_eq!(
            required_permission("POST", "/v1/atlas/dr/failover"),
            Some(ApiPermission::ClusterAdmin)
        );
        // Other DR writes (peer register/delete, mirror enable/disable, rpo)
        // stay at the general Atlas storage.admin tier.
        assert_eq!(
            required_permission("POST", "/v1/atlas/dr/peers"),
            Some(ApiPermission::StorageAdmin)
        );
        assert_eq!(
            required_permission("POST", "/v1/atlas/volumes/vol_1/mirror"),
            Some(ApiPermission::StorageAdmin)
        );
    }

    #[test]
    fn pods_and_namespaces_require_cluster_admin() {
        for (method, path) in [
            ("GET", "/v1/pods"),
            ("GET", "/v1/pods/"),
            ("GET", "/v1/pods?namespace=kube-system"),
            ("POST", "/v1/pods/default/web/exec"),
            ("DELETE", "/v1/pods/default/web"),
            ("POST", "/v1/pods/default/web/restart"),
            ("GET", "/v1/pods/default/web/events"),
            ("GET", "/v1/pods/default/web/yaml"),
            ("GET", "/v1/pods/capabilities"),
            ("GET", "/v1/namespaces"),
        ] {
            assert_eq!(
                required_permission(method, path),
                Some(ApiPermission::ClusterAdmin),
                "{method} {path}"
            );
        }
        assert!(!role_has_permission(
            &Role::User,
            ApiPermission::ClusterAdmin
        ));
        assert!(!role_has_permission(
            &Role::Viewer,
            ApiPermission::ClusterAdmin
        ));
    }

    #[test]
    fn rescue_mode_requires_cluster_admin_even_on_get() {
        for (method, path) in [
            ("POST", "/vms/web-01/rescue"),
            ("GET", "/vms/web-01/rescue/zorvia-rescue-web-01-abc123"),
            ("DELETE", "/vms/web-01/rescue/zorvia-rescue-web-01-abc123"),
        ] {
            assert_eq!(
                required_permission(method, path),
                Some(ApiPermission::ClusterAdmin),
                "{method} {path}"
            );
        }
        // A VM's other routes stay under the normal fabric rules -- this
        // isn't a blanket "everything under /vms/ is admin-only" change.
        assert_eq!(required_permission("GET", "/vms/web-01"), None);
        assert_eq!(
            required_permission("POST", "/vms/web-01/start"),
            Some(ApiPermission::VmPower)
        );
    }
}
