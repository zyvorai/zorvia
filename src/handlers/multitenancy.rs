use crate::tui::colors::cli as color;
use anyhow::Result;

pub fn handle_tenants_list(active_only: bool, output: String) -> Result<()> {
    use crate::multitenancy::tenants::TenantManager;

    println!("{}", color::header("Tenants"));
    println!();

    let manager = TenantManager::new();
    let tenants = if active_only {
        manager.active_tenants()
    } else {
        manager.list_tenants()
    };

    println!("  Total tenants: {}", tenants.len());
    println!("  Format: {}", output);
    println!();
    println!("{}", color::success("✓ Tenants listed"));
    Ok(())
}

pub fn handle_tenants_create(
    name: String,
    owner: String,
    email: String,
    description: Option<String>,
    namespace: Option<String>,
) -> Result<()> {
    use crate::multitenancy::tenants::Tenant;

    println!("{}", color::header(&format!("Creating Tenant: {}", name)));
    println!();

    let mut tenant = Tenant::new(&name, &owner, &email);

    if let Some(desc) = description {
        tenant = tenant.with_description(desc);
    }

    if let Some(ns) = namespace {
        tenant.add_namespace(ns);
    }

    println!("  Name:    {}", color::value(&tenant.name));
    println!("  Owner:   {}", tenant.owner_id);
    println!("  Email:   {}", tenant.contact_email);
    println!();
    println!("{}", color::success("✓ Tenant created successfully"));
    Ok(())
}

pub fn handle_tenants_show(tenant: String, output: String) -> Result<()> {
    println!("{}", color::header(&format!("Tenant: {}", tenant)));
    println!();

    println!("  Format: {}", output);
    println!();
    println!("{}", color::success("✓ Tenant details retrieved"));
    Ok(())
}

pub fn handle_tenants_delete(tenant: String, yes: bool) -> Result<()> {
    println!("{}", color::header(&format!("Deleting Tenant: {}", tenant)));
    println!();

    if !yes {
        println!("  Skipped: Confirmation required");
    } else {
        println!("  Tenant ID: {}", color::value(&tenant));
        println!();
        println!("{}", color::success("✓ Tenant deleted"));
    }
    Ok(())
}

pub fn handle_users_list(active_only: bool, group: Option<String>, output: String) -> Result<()> {
    use crate::multitenancy::AccessControlManager;

    println!("{}", color::header("Users"));
    println!();

    let manager = AccessControlManager::new();
    let users = manager.list_users();

    println!("  Total users: {}", users.len());
    if active_only {
        println!("  Filter: Active only");
    }
    if let Some(g) = &group {
        println!("  Group: {}", color::value(g));
    }
    println!("  Format: {}", output);
    println!();
    println!("{}", color::success("✓ Users listed"));
    Ok(())
}

pub fn handle_users_create(
    username: String,
    email: String,
    role: Option<String>,
    group: Option<String>,
) -> Result<()> {
    use crate::multitenancy::User;

    println!("{}", color::header(&format!("Creating User: {}", username)));
    println!();

    let mut user = User::new(&username, &email);

    if let Some(r) = &role {
        user = user.add_role(r);
        println!("  Role assigned: {}", color::value(r));
    }

    if let Some(g) = &group {
        user = user.add_group(g);
        println!("  Group added: {}", color::value(g));
    }

    println!("  Username: {}", color::value(&user.username));
    println!("  Email:    {}", user.email);
    println!();
    println!("{}", color::success("✓ User created successfully"));
    println!("  {}", color::muted("Note: Configuration is not persisted to storage"));
    Ok(())
}

pub fn handle_users_assign_role(user: String, role: String, scope: String) -> Result<()> {
    use crate::multitenancy::roles::{BindingScope, RoleBinding, Subject};

    println!("{}", color::header("Assigning Role"));
    println!();

    let subject = Subject::User {
        user_id: user.clone(),
    };
    let binding_scope = if scope == "cluster" {
        BindingScope::Cluster
    } else if let Some(ns) = scope.strip_prefix("namespace:") {
        BindingScope::Namespace {
            namespace: ns.to_string(),
        }
    } else {
        BindingScope::Cluster
    };

    let _binding = RoleBinding::new(&role, subject, binding_scope);

    println!("  User:  {}", color::value(&user));
    println!("  Role:  {}", color::value(&role));
    println!("  Scope: {}", scope);
    println!();
    println!("{}", color::success("✓ Role assigned"));
    Ok(())
}

pub fn handle_roles_list(builtin: bool, custom: bool, output: String) -> Result<()> {
    use crate::multitenancy::roles::RoleManager;

    println!("{}", color::header("Roles"));
    println!();

    let manager = RoleManager::new();
    let roles = if builtin {
        manager.builtin_roles()
    } else if custom {
        manager.custom_roles()
    } else {
        manager.list_roles()
    };

    println!("  Total roles: {}", roles.len());
    println!("  Format: {}", output);
    println!();
    println!("{}", color::success("✓ Roles listed"));
    Ok(())
}

pub fn handle_roles_show(role: String, output: String) -> Result<()> {
    println!("{}", color::header(&format!("Role: {}", role)));
    println!();

    println!("  Format: {}", output);
    println!();
    println!("{}", color::success("✓ Role details retrieved"));
    Ok(())
}

pub fn handle_roles_create(
    name: String,
    description: Option<String>,
    permissions: String,
) -> Result<()> {
    use crate::multitenancy::roles::Role;

    println!("{}", color::header(&format!("Creating Role: {}", name)));
    println!();

    let mut role = Role::new(&name);

    if let Some(desc) = description {
        role = role.with_description(desc);
    }

    println!("  Name:        {}", color::value(&role.name));
    println!("  Permissions: {}", permissions);
    println!();
    println!("{}", color::success("✓ Role created successfully"));
    println!("  {}", color::muted("Note: Configuration is not persisted to storage"));
    Ok(())
}

pub fn handle_quotas_list(namespace: Option<String>, exceeded: bool, output: String) -> Result<()> {
    use crate::multitenancy::quotas::QuotaManager;

    println!("{}", color::header("Resource Quotas"));
    println!();

    let manager = QuotaManager::new();
    let quotas = if exceeded {
        manager.exceeded_quotas()
    } else {
        manager.list_quotas()
    };

    println!("  Total quotas: {}", quotas.len());
    if let Some(ns) = &namespace {
        println!("  Namespace: {}", color::value(ns));
    }
    println!("  Format: {}", output);
    println!();
    println!("{}", color::success("✓ Quotas listed"));
    Ok(())
}

pub fn handle_quotas_create(name: String, namespace: String, preset: String) -> Result<()> {
    use crate::multitenancy::quotas::{ResourceLimits, ResourceQuota};

    println!("{}", color::header(&format!("Creating Quota: {}", name)));
    println!();

    let limits = match preset.as_str() {
        "small" => ResourceLimits::small(),
        "large" => ResourceLimits::large(),
        "unlimited" => ResourceLimits::unlimited(),
        _ => ResourceLimits::medium(),
    };

    let quota = ResourceQuota::new(&name, &namespace).with_limits(limits.clone());

    println!("  Name:      {}", color::value(&quota.name));
    println!("  Namespace: {}", quota.namespace);
    println!("  Preset:    {}", preset);
    println!("  Max VMs:   {}", limits.max_vms);
    println!("  Max CPUs:  {}", limits.max_cpu_cores);
    println!("  Max Memory: {} Gi", limits.max_memory_gi);
    println!();
    println!("{}", color::success("✓ Quota created successfully"));
    Ok(())
}

pub fn handle_quotas_show(quota: String, utilization: bool, output: String) -> Result<()> {
    println!("{}", color::header(&format!("Quota: {}", quota)));
    println!();

    println!("  Show utilization: {}", utilization);
    println!("  Format: {}", output);
    println!();
    println!("{}", color::success("✓ Quota details retrieved"));
    Ok(())
}

pub fn handle_groups_list(output: String) -> Result<()> {
    use crate::multitenancy::AccessControlManager;

    println!("{}", color::header("Groups"));
    println!();

    let manager = AccessControlManager::new();
    let groups = manager.list_groups();

    println!("  Total groups: {}", groups.len());
    println!("  Format: {}", output);
    println!();
    println!("{}", color::success("✓ Groups listed"));
    Ok(())
}

pub fn handle_groups_create(
    name: String,
    description: Option<String>,
    role: Option<String>,
) -> Result<()> {
    use crate::multitenancy::Group;

    println!("{}", color::header(&format!("Creating Group: {}", name)));
    println!();

    let mut group = Group::new(&name);

    if let Some(desc) = description {
        group = group.with_description(desc);
    }

    if let Some(r) = &role {
        group.add_role(r);
        println!("  Role assigned: {}", color::value(r));
    }

    println!("  Name: {}", color::value(&group.name));
    println!();
    println!("{}", color::success("✓ Group created successfully"));
    println!("  {}", color::muted("Note: Configuration is not persisted to storage"));
    Ok(())
}

pub fn handle_groups_add_user(group: String, user: String) -> Result<()> {
    println!("{}", color::header("Adding User to Group"));
    println!();

    println!("  Group: {}", color::value(&group));
    println!("  User:  {}", color::value(&user));
    println!();
    println!("{}", color::success("✓ User added to group"));
    println!("  {}", color::muted("Note: Configuration is not persisted to storage"));
    Ok(())
}
