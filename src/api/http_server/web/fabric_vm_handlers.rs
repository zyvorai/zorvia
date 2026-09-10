//! Fabric-compat VM create / images / port-forwards / cloud-init / clone / metrics.

use super::*;
use crate::config::{
    validate_vm_config, DiskConfig, FeaturesConfig, FirmwareConfig, InterfaceConfig,
    VMConfigBuilder,
};
use crate::snapshots::{SnapshotConfig, SnapshotManager};
use axum::extract::Json as AxumJson;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct FabricCreateVmRequest {
    pub name: String,
    pub image: String,
    pub cpus: u32,
    pub memory: u64,
    #[serde(default)]
    pub disk: Option<u64>,
    #[serde(default)]
    pub port_forwards: Option<Vec<FabricPortForward>>,
    #[serde(default)]
    pub network_tap: Option<bool>,
    #[serde(default)]
    pub network_static_ip: Option<bool>,
    #[serde(default)]
    pub tenant: Option<String>,
    #[serde(default)]
    pub labels: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub guest_os: Option<String>,
    #[serde(default)]
    pub cloud_init: Option<FabricCloudInit>,
    #[serde(default)]
    pub expose_ssh: Option<bool>,
    #[serde(default)]
    pub expose_vnc: Option<bool>,
    #[serde(default)]
    pub expose_rdp: Option<bool>,
    #[serde(default)]
    pub start: Option<bool>,
    /// Maximum socket count this VM can be CPU-hotplugged up to. Defaults to
    /// 4x the initial socket count when unset. Must be declared at create
    /// time since KubeVirt cannot raise it later.
    #[serde(default)]
    pub cpu_max_sockets: Option<u32>,
    /// Maximum guest memory (MiB) this VM can be memory-hotplugged up to.
    /// Defaults to 2x the initial memory when unset.
    #[serde(default)]
    pub memory_max_guest_mb: Option<u64>,

    // ── Advanced (Phase 2): full VMConfig surface, all optional ──
    /// CPU model (e.g. "host-passthrough", "Haswell").
    #[serde(default)]
    pub cpu_model: Option<String>,
    /// Pin vCPUs to physical CPUs for latency-sensitive workloads.
    #[serde(default)]
    pub cpu_dedicated_placement: Option<bool>,
    /// Isolate the QEMU emulator thread from vCPU threads.
    #[serde(default)]
    pub cpu_isolate_emulator_thread: Option<bool>,
    /// Hugepages page size (e.g. "2Mi", "1Gi").
    #[serde(default)]
    pub memory_hugepages_page_size: Option<String>,
    /// Firmware/bootloader: `{"bootloader": "bios"}` or
    /// `{"bootloader": {"efi": {"secure_boot": true, "persistent": true}}}`.
    #[serde(default)]
    pub firmware: Option<FirmwareConfig>,
    /// Domain features: ACPI/APIC/HyperV enlightenments/SMM.
    #[serde(default)]
    pub features: Option<FeaturesConfig>,
    /// Machine type (e.g. "q35").
    #[serde(default)]
    pub machine_type: Option<String>,
    /// Attach a TPM 2.0 device.
    #[serde(default)]
    pub enable_tpm: Option<bool>,
    /// Attach a virtio-rng device.
    #[serde(default)]
    pub enable_rng: Option<bool>,
    /// Full disk list, overriding the single-disk `image`/`disk` shorthand
    /// when present. Each entry matches `VMConfig`'s `DiskConfig` JSON shape
    /// (the same shape the CLI's `--from-file` accepts).
    #[serde(default)]
    pub disks: Option<Vec<DiskConfig>>,
    /// Full interface list, overriding the single-NIC `network_tap` shorthand
    /// when present. Each entry matches `VMConfig`'s `InterfaceConfig` shape.
    #[serde(default)]
    pub interfaces: Option<Vec<InterfaceConfig>>,
}

#[derive(Debug, Deserialize)]
pub struct FabricCloudInit {
    #[serde(default)]
    pub user_data: Option<String>,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub ssh_authorized_keys: Option<Vec<String>>,
    #[serde(default)]
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FabricPortForward {
    pub host_port: i32,
    pub guest_port: i32,
    #[serde(default = "default_tcp")]
    pub protocol: String,
}

fn default_tcp() -> String {
    "tcp".into()
}

#[derive(Debug, Deserialize)]
pub struct CloudInitPostBody {
    #[serde(default)]
    pub instance_id: Option<String>,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub user_data: Option<String>,
    #[serde(default)]
    pub meta_data: Option<serde_json::Value>,
    #[serde(default)]
    pub network_config: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CloneBody {
    pub target_name: String,
    #[serde(default)]
    pub include_snapshots: Option<bool>,
    #[serde(default)]
    pub linked_clone: Option<bool>,
    /// `cdi` (default) creates a CDI DataVolume from the source PVC.
    /// `empty` allocates a blank same-size PVC.
    #[serde(default)]
    pub clone_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CloudDownloadBody {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateSnapshotBody {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub snapshot_type: Option<String>,
}

fn build_cloud_init_yaml(ci: &FabricCloudInit, hostname_fallback: &str) -> String {
    if let Some(ud) = ci.user_data.as_ref().filter(|s| !s.trim().is_empty()) {
        return ud.clone();
    }
    let hostname = ci
        .hostname
        .clone()
        .unwrap_or_else(|| hostname_fallback.to_string());
    let username = ci.username.clone().unwrap_or_else(|| "zorvia".to_string());
    let mut lines = vec![
        "#cloud-config".to_string(),
        format!("hostname: {hostname}"),
        "manage_etc_hosts: true".to_string(),
        "users:".to_string(),
        format!("  - name: {username}"),
        "    sudo: ALL=(ALL) NOPASSWD:ALL".to_string(),
        "    shell: /bin/bash".to_string(),
        "    groups: sudo".to_string(),
    ];
    if let Some(keys) = &ci.ssh_authorized_keys {
        if !keys.is_empty() {
            lines.push("    ssh_authorized_keys:".into());
            for k in keys {
                lines.push(format!("      - {k}"));
            }
        }
    }
    if let Some(pw) = &ci.password {
        if !pw.is_empty() {
            lines.push("chpasswd:".into());
            lines.push("  list: |".into());
            lines.push(format!("    {username}:{pw}"));
            lines.push("  expire: false".into());
            lines.push("ssh_pwauth: true".into());
        }
    }
    lines.push("package_update: false".into());
    lines.join("\n")
}

fn memory_to_kube(mib: u64) -> String {
    if mib >= 1024 && mib % 1024 == 0 {
        format!("{}Gi", mib / 1024)
    } else {
        format!("{}Mi", mib)
    }
}

pub async fn fabric_list_images() -> impl IntoResponse {
    // Ready quay.io/containerdisks catalog (major Linux) + blank disks.
    // See docs/GOLDEN_IMAGES.md for CDI golden import of the same sources.
    Json(json!([
        {
            "name": "Blank disk (20 Gi)",
            "path": "blank:20Gi",
            "format": "blank",
            "size_bytes": 21474836480u64
        },
        {
            "name": "Blank disk (40 Gi)",
            "path": "blank:40Gi",
            "format": "blank",
            "size_bytes": 42949672960u64
        },
        {
            "name": "Ubuntu 24.04 (containerdisk)",
            "path": "quay.io/containerdisks/ubuntu:24.04",
            "format": "containerdisk",
            "size_bytes": 0
        },
        {
            "name": "Ubuntu 22.04 (containerdisk)",
            "path": "quay.io/containerdisks/ubuntu:22.04",
            "format": "containerdisk",
            "size_bytes": 0
        },
        {
            "name": "Ubuntu 20.04 (containerdisk)",
            "path": "quay.io/containerdisks/ubuntu:20.04",
            "format": "containerdisk",
            "size_bytes": 0
        },
        {
            "name": "Ubuntu 18.04 (containerdisk)",
            "path": "quay.io/containerdisks/ubuntu:18.04",
            "format": "containerdisk",
            "size_bytes": 0
        },
        {
            "name": "Fedora 41 (containerdisk)",
            "path": "quay.io/containerdisks/fedora:41",
            "format": "containerdisk",
            "size_bytes": 0
        },
        {
            "name": "Fedora 40 (containerdisk)",
            "path": "quay.io/containerdisks/fedora:40",
            "format": "containerdisk",
            "size_bytes": 0
        },
        {
            "name": "Fedora 39 (containerdisk)",
            "path": "quay.io/containerdisks/fedora:39",
            "format": "containerdisk",
            "size_bytes": 0
        },
        {
            "name": "CentOS Stream 9 (containerdisk)",
            "path": "quay.io/containerdisks/centos-stream:9",
            "format": "containerdisk",
            "size_bytes": 0
        },
        {
            "name": "CentOS Stream 8 (containerdisk)",
            "path": "quay.io/containerdisks/centos-stream:8",
            "format": "containerdisk",
            "size_bytes": 0
        },
        {
            "name": "Debian 12 (containerdisk)",
            "path": "quay.io/containerdisks/debian:12",
            "format": "containerdisk",
            "size_bytes": 0
        },
        {
            "name": "Debian 11 (containerdisk)",
            "path": "quay.io/containerdisks/debian:11",
            "format": "containerdisk",
            "size_bytes": 0
        },
        {
            "name": "AlmaLinux 9 (containerdisk)",
            "path": "quay.io/containerdisks/almalinux:9",
            "format": "containerdisk",
            "size_bytes": 0
        },
        {
            "name": "AlmaLinux 8 (containerdisk)",
            "path": "quay.io/containerdisks/almalinux:8",
            "format": "containerdisk",
            "size_bytes": 0
        },
        {
            "name": "Rocky Linux 9 (containerdisk)",
            "path": "quay.io/containerdisks/rockylinux:9",
            "format": "containerdisk",
            "size_bytes": 0
        },
        {
            "name": "Rocky Linux 8 (containerdisk)",
            "path": "quay.io/containerdisks/rockylinux:8",
            "format": "containerdisk",
            "size_bytes": 0
        },
        {
            "name": "Alpine 3.19 (containerdisk)",
            "path": "quay.io/containerdisks/alpine:3.19",
            "format": "containerdisk",
            "size_bytes": 0
        }
    ]))
}

pub async fn fabric_list_cloud_images() -> impl IntoResponse {
    let images = crate::kube::catalog::cloud_images_from_templates();
    Json(json!(images))
}

pub async fn fabric_list_downloads() -> impl IntoResponse {
    let jobs = crate::golden_images::jobs::DownloadRegistry::global().list();
    Json(json!(jobs))
}

pub async fn fabric_start_download(
    State(state): State<SharedState>,
    AxumJson(body): AxumJson<CloudDownloadBody>,
) -> impl IntoResponse {
    let s = state.read().await;
    let namespace = s.namespace.clone();
    let client = s.client();
    drop(s);
    match crate::golden_images::jobs::DownloadRegistry::global()
        .start(&body.name, &namespace, &client)
        .await
    {
        Ok(job) => (StatusCode::ACCEPTED, Json(json!(job))).into_response(),
        Err(e) => {
            let (st, j) = err_json(404, "NOT_FOUND", &e.to_string());
            (st, j).into_response()
        }
    }
}

pub async fn fabric_create_vm(
    State(state): State<SharedState>,
    AxumJson(req): AxumJson<FabricCreateVmRequest>,
) -> impl IntoResponse {
    let s = state.read().await;
    let namespace = s.namespace.clone();
    let client = s.client();
    drop(s);

    if req.name.is_empty() {
        let (st, j) = err_json(400, "INVALID", "VM name is required");
        return (st, j).into_response();
    }

    let guest_os = req
        .guest_os
        .as_deref()
        .unwrap_or("linux")
        .to_ascii_lowercase();
    let disk_gb = req.disk.unwrap_or(20).max(1);
    let disk_size = format!("{disk_gb}Gi");
    let mem = memory_to_kube(req.memory.max(256));
    let cpus = req.cpus.max(1);

    let max_sockets = req
        .cpu_max_sockets
        .unwrap_or_else(|| cpus.saturating_mul(4))
        .max(1);
    let max_guest_mib = req.memory_max_guest_mb.unwrap_or(req.memory.max(256) * 2);

    let mut builder = VMConfigBuilder::new(&req.name)
        .namespace(&namespace)
        .cpu(cpus, 1, 1)
        .cpu_max_sockets(max_sockets)
        .memory(&mem)
        .max_guest_memory(memory_to_kube(max_guest_mib));

    if let Some(model) = &req.cpu_model {
        builder = builder.cpu_model(model);
    }
    if let Some(dedicated) = req.cpu_dedicated_placement {
        builder = builder.dedicated_cpu_placement(dedicated);
    }
    if let Some(isolate) = req.cpu_isolate_emulator_thread {
        builder = builder.isolate_emulator_thread(isolate);
    }
    if let Some(page_size) = &req.memory_hugepages_page_size {
        builder = builder.hugepages(page_size);
    }
    if let Some(machine_type) = &req.machine_type {
        builder = builder.machine_type(machine_type);
    }
    if let Some(firmware) = req.firmware.clone() {
        builder = builder.firmware(firmware);
    }
    if let Some(features) = req.features.clone() {
        builder = builder.features(features);
    }
    if req.enable_tpm.unwrap_or(false) {
        builder = builder.enable_tpm();
    }
    if req.enable_rng.unwrap_or(false) {
        builder = builder.enable_rng();
    }

    if let Some(disks) = req.disks.clone().filter(|d| !d.is_empty()) {
        for disk in disks {
            builder = builder.add_disk(disk);
        }
    } else {
        let image = req.image.trim();
        if image.is_empty() || image.starts_with("blank:") {
            let size = image
                .strip_prefix("blank:")
                .unwrap_or(&disk_size)
                .to_string();
            builder = builder.add_blank_disk("rootdisk", size, 1);
        } else if let Some(pvc) = image.strip_prefix("pvc:") {
            builder = builder.add_pvc_disk("rootdisk", pvc, &disk_size, 1);
        } else if let Some(dv) = image.strip_prefix("datavolume:") {
            // A golden image imported via the cloud-image download flow
            // (DownloadJob.output_path) — an already-created CDI DataVolume.
            builder = builder.add_data_volume_disk("rootdisk", dv, &disk_size, 1);
        } else if image.contains('/') || image.contains(':') {
            // OCI containerdisk reference
            builder = builder.add_container_disk("rootdisk", image, 1);
        } else {
            builder = builder.add_blank_disk("rootdisk", &disk_size, 1);
        }
    }

    if let Some(interfaces) = req.interfaces.clone().filter(|i| !i.is_empty()) {
        for iface in interfaces {
            builder = builder.add_interface(iface);
        }
    } else if req.network_tap.unwrap_or(false) {
        builder = builder.add_bridge_network("default");
    } else {
        builder = builder.add_pod_network("default");
    }

    if guest_os != "windows" {
        if let Some(ci) = &req.cloud_init {
            let ud = build_cloud_init_yaml(ci, &req.name);
            builder = builder.cloud_init(ud);
        } else {
            // Minimal Linux cloud-init so images that expect a datasource still boot.
            let ud = format!(
                "#cloud-config\nhostname: {}\nmanage_etc_hosts: true\nusers:\n  - name: zorvia\n    sudo: ALL=(ALL) NOPASSWD:ALL\n    shell: /bin/bash\n",
                req.name
            );
            builder = builder.cloud_init(ud);
        }
    }

    if let Some(labels) = &req.labels {
        for (k, v) in labels {
            builder = builder.label(k, v);
        }
    }
    if let Some(tenant) = &req.tenant {
        builder = builder.label("tenant", tenant);
    }
    builder = builder.label("zorvia.io/guest-os", &guest_os);

    let config = builder.build();
    if let Err(e) = validate_vm_config(&config) {
        let (st, j) = err_json(400, "INVALID_CONFIG", &e.to_string());
        return (st, j).into_response();
    }

    match client.create_vm(&config).await {
        Ok(_vm) => {
            let start = req.start.unwrap_or(true);
            if start {
                let _ = client.start_vm(&namespace, &req.name).await;
                if std::env::var("ZORVIA_SKIP_GUEST_WAIT").ok().as_deref() != Some("1") {
                    let _ = client
                        .wait_until_guest_ready(&namespace, &req.name, guest_wait_secs())
                        .await;
                }
            }

            // Port forwards + convenience expose flags
            let mut forwards = req.port_forwards.unwrap_or_default();
            if req.expose_ssh.unwrap_or(false) {
                forwards.push(FabricPortForward {
                    host_port: 0,
                    guest_port: 22,
                    protocol: "tcp".into(),
                });
            }
            if req.expose_vnc.unwrap_or(guest_os == "windows") {
                forwards.push(FabricPortForward {
                    host_port: 0,
                    guest_port: 5900,
                    protocol: "tcp".into(),
                });
            }
            if req.expose_rdp.unwrap_or(false) {
                forwards.push(FabricPortForward {
                    host_port: 0,
                    guest_port: 3389,
                    protocol: "tcp".into(),
                });
            }

            let mut pf_infos = Vec::new();
            for f in &forwards {
                let preferred = if f.host_port >= 30000 {
                    Some(f.host_port)
                } else {
                    None
                };
                match client
                    .add_port_forward(&namespace, &req.name, f.guest_port, &f.protocol, preferred)
                    .await
                {
                    Ok(info) => pf_infos.push(info),
                    Err(e) => {
                        log::warn!("port-forward {}:{} failed: {e}", f.guest_port, f.protocol)
                    }
                }
            }

            // The VM was just created; a GET by name right after can transiently
            // fail (K8s API read-after-write lag), which would otherwise report
            // "creation failed" for a VM that actually exists. Retry briefly
            // before surfacing an error to the client.
            let mut get_result = client.get_vm(&namespace, &req.name).await;
            for _ in 0..3 {
                if get_result.is_ok() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                get_result = client.get_vm(&namespace, &req.name).await;
            }
            match get_result {
                Ok(vm) => {
                    let ip = client
                        .get_vm_ip(&namespace, &req.name)
                        .await
                        .unwrap_or(None);
                    let mut body = fabric_vm_json(&VmInfo::from_vm_with_ip(&vm, ip));
                    if let Some(obj) = body.as_object_mut() {
                        if let Ok(ready) = client.guest_ready_report(&namespace, &req.name).await {
                            obj.insert("guest_ready".into(), json!(ready));
                        }
                        obj.insert(
                            "port_forwards".into(),
                            json!(pf_infos
                                .iter()
                                .map(|p| json!({
                                    "host_port": p.host_port,
                                    "guest_port": p.guest_port,
                                    "protocol": p.protocol,
                                    "node_port": p.node_port,
                                    "expose_host": p.expose_host,
                                }))
                                .collect::<Vec<_>>()),
                        );
                    }
                    (StatusCode::CREATED, Json(body)).into_response()
                }
                Err(e) => {
                    let (st, j) = err_json(500, "CREATE_FAILED", &sanitize_error(&e));
                    (st, j).into_response()
                }
            }
        }
        Err(e) => {
            log::error!("create_vm failed: {e}");
            // ZorviaError's own Display text (e.g. "VM 'x' already exists")
            // is safe to return as-is — it's our own message, not a raw k8s
            // error — but it doesn't start with any of sanitize_error's
            // allowed prefixes, so passing it through sanitize_error first
            // collapsed it to a generic "Internal server error" and (since
            // that check ran on the already-sanitized text) always reported
            // 500 instead of 409 for a name conflict.
            let (code, msg) = match e.downcast_ref::<crate::utils::ZorviaError>() {
                Some(crate::utils::ZorviaError::VmExists(_)) => (409, e.to_string()),
                Some(crate::utils::ZorviaError::VmNotFound(_)) => (404, e.to_string()),
                Some(crate::utils::ZorviaError::ValidationError(_))
                | Some(crate::utils::ZorviaError::ConfigError(_)) => (400, e.to_string()),
                Some(crate::utils::ZorviaError::Timeout(_)) => (504, e.to_string()),
                _ => (500, sanitize_error(&e)),
            };
            let (st, j) = err_json(code, "CREATE_FAILED", &msg);
            (st, j).into_response()
        }
    }
}

pub async fn fabric_add_port_forward(
    State(state): State<SharedState>,
    Path(name): Path<String>,
    AxumJson(body): AxumJson<FabricPortForward>,
) -> impl IntoResponse {
    let s = state.read().await;
    let namespace = s.namespace.clone();
    let client = s.client();
    drop(s);
    let preferred = if body.host_port >= 30000 {
        Some(body.host_port)
    } else {
        None
    };
    match client
        .add_port_forward(
            &namespace,
            &name,
            body.guest_port,
            &body.protocol,
            preferred,
        )
        .await
    {
        Ok(_) => match client.get_vm(&namespace, &name).await {
            Ok(vm) => {
                let ip = client.get_vm_ip(&namespace, &name).await.unwrap_or(None);
                let mut body = fabric_vm_json(&VmInfo::from_vm_with_ip(&vm, ip));
                if let Ok(pfs) = client.list_port_forwards(&namespace, &name).await {
                    if let Some(obj) = body.as_object_mut() {
                        obj.insert(
                            "port_forwards".into(),
                            json!(pfs
                                .iter()
                                .map(|p| json!({
                                    "host_port": p.host_port,
                                    "guest_port": p.guest_port,
                                    "protocol": p.protocol,
                                    "expose_host": p.expose_host,
                                }))
                                .collect::<Vec<_>>()),
                        );
                    }
                }
                Json(body).into_response()
            }
            Err(e) => {
                let (st, j) = err_json(404, "NOT_FOUND", &sanitize_error(&e));
                (st, j).into_response()
            }
        },
        Err(e) => {
            let (st, j) = err_json(500, "PORT_FORWARD_FAILED", &sanitize_error(&e));
            (st, j).into_response()
        }
    }
}

pub async fn fabric_remove_port_forward(
    State(state): State<SharedState>,
    Path((name, host_port)): Path<(String, i32)>,
) -> impl IntoResponse {
    let s = state.read().await;
    let namespace = s.namespace.clone();
    let client = s.client();
    drop(s);
    match client
        .remove_port_forward(&namespace, &name, host_port)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            let (st, j) = err_json(404, "NOT_FOUND", &sanitize_error(&e));
            (st, j).into_response()
        }
    }
}

pub async fn fabric_cloud_init(
    State(state): State<SharedState>,
    Path(name): Path<String>,
    AxumJson(body): AxumJson<CloudInitPostBody>,
) -> impl IntoResponse {
    let s = state.read().await;
    let namespace = s.namespace.clone();
    let client = s.client();
    drop(s);

    let user_data = body.user_data.unwrap_or_else(|| {
        let host = body.hostname.unwrap_or_else(|| name.clone());
        format!("#cloud-config\nhostname: {host}\nmanage_etc_hosts: true\n")
    });

    // Annotate VM with desired cloud-init for operators; full volume rewrite requires stop/recreate.
    let patch = json!({
        "metadata": {
            "annotations": {
                "zorvia.io/cloud-init-user-data": user_data,
                "zorvia.io/cloud-init-instance-id": body.instance_id.unwrap_or_else(|| name.clone()),
            }
        }
    });

    let vms: kube::Api<crate::kube::types::VirtualMachine> =
        kube::Api::namespaced(client.client(), &namespace);
    match vms
        .patch(
            &name,
            &kube::api::PatchParams::default(),
            &kube::api::Patch::Merge(&patch),
        )
        .await
    {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            let (st, j) = err_json(500, "CLOUD_INIT_FAILED", &format!("{e}"));
            (st, j).into_response()
        }
    }
}

pub async fn fabric_clone_vm(
    State(state): State<SharedState>,
    Path(source): Path<String>,
    AxumJson(body): AxumJson<CloneBody>,
) -> impl IntoResponse {
    let s = state.read().await;
    let namespace = s.namespace.clone();
    let client = s.client();
    drop(s);

    let src = match client.get_vm(&namespace, &source).await {
        Ok(vm) => vm,
        Err(e) => {
            let (st, j) = err_json(404, "NOT_FOUND", &sanitize_error(&e));
            return (st, j).into_response();
        }
    };

    let spec = &src.spec.template.spec;
    let domain = &spec.domain;
    let cpu_cores = domain.cpu.as_ref().and_then(|c| c.cores).unwrap_or(1);
    let memory = domain
        .memory
        .as_ref()
        .and_then(|m| m.guest.clone())
        .unwrap_or_else(|| "2Gi".into());

    let mut builder = VMConfigBuilder::new(&body.target_name)
        .namespace(&namespace)
        .cpu(cpu_cores, 1, 1)
        .memory(&memory)
        .add_pod_network("default");

    let mut added_disk = false;
    let mut pvc_notes = Vec::new();
    let mut pending_dvs: Vec<String> = Vec::new();
    if let Some(volumes) = &spec.volumes {
        for (i, vol) in volumes.iter().enumerate() {
            let order = (i as u32).saturating_add(1);
            if let Some(empty) = &vol.empty_disk {
                builder = builder.add_blank_disk(&vol.name, &empty.capacity, order);
                added_disk = true;
            } else if let Some(cd) = &vol.container_disk {
                builder = builder.add_container_disk(&vol.name, &cd.image, order);
                added_disk = true;
            } else if let Some(pvc) = &vol.persistent_volume_claim {
                let clone_name = crate::kube::cdi::clone_dv_name(&body.target_name, &vol.name);
                let size = match client.get_pvc(&namespace, &pvc.claim_name).await {
                    Ok(src_pvc) => src_pvc
                        .spec
                        .as_ref()
                        .and_then(|s| s.resources.as_ref())
                        .and_then(|r| r.requests.as_ref())
                        .and_then(|req| req.get("storage"))
                        .map(|q| q.0.clone())
                        .unwrap_or_else(|| "20Gi".into()),
                    Err(_) => "20Gi".into(),
                };
                let mode = body
                    .clone_mode
                    .as_deref()
                    .unwrap_or("cdi")
                    .to_ascii_lowercase();
                if mode == "cdi" {
                    let spec = crate::kube::cdi::CdiPvcCloneSpec {
                        source_namespace: namespace.clone(),
                        source_pvc: pvc.claim_name.clone(),
                        target_namespace: namespace.clone(),
                        target_name: clone_name.clone(),
                        size: size.clone(),
                        storage_class: None,
                    };
                    match crate::kube::cdi::data_volume_clone_manifest(&spec) {
                        Ok(manifest) => match client.apply_data_volume(&namespace, &manifest).await
                        {
                            Ok(_) => {
                                builder =
                                    builder.add_pvc_disk(&vol.name, &clone_name, &size, order);
                                added_disk = true;
                                pvc_notes.push(format!(
                                    "CDI DataVolume {clone_name} cloning PVC {}",
                                    pvc.claim_name
                                ));
                                pending_dvs.push(clone_name.clone());
                            }
                            Err(e) => {
                                log::warn!(
                                    "CDI clone {} failed, empty PVC fallback: {e}",
                                    pvc.claim_name
                                );
                                if client
                                    .create_pvc(&namespace, &clone_name, &size, None)
                                    .await
                                    .is_ok()
                                {
                                    builder =
                                        builder.add_pvc_disk(&vol.name, &clone_name, &size, order);
                                    added_disk = true;
                                }
                                pvc_notes.push(format!(
                                    "CDI unavailable for {}: {}; allocated empty PVC",
                                    pvc.claim_name,
                                    sanitize_error(&e)
                                ));
                            }
                        },
                        Err(e) => pvc_notes.push(format!("invalid CDI spec: {e}")),
                    }
                } else if client
                    .create_pvc(&namespace, &clone_name, &size, None)
                    .await
                    .is_ok()
                {
                    builder = builder.add_pvc_disk(&vol.name, &clone_name, &size, order);
                    added_disk = true;
                    pvc_notes.push(format!("allocated empty PVC {clone_name} ({size})"));
                }
            } else if let Some(dv) = &vol.data_volume {
                let clone_name = crate::kube::cdi::clone_dv_name(&body.target_name, &vol.name);
                let spec = crate::kube::cdi::CdiPvcCloneSpec {
                    source_namespace: namespace.clone(),
                    source_pvc: dv.name.clone(),
                    target_namespace: namespace.clone(),
                    target_name: clone_name.clone(),
                    size: "20Gi".into(),
                    storage_class: None,
                };
                if let Ok(manifest) = crate::kube::cdi::data_volume_clone_manifest(&spec) {
                    match client.apply_data_volume(&namespace, &manifest).await {
                        Ok(_) => {
                            builder = builder.add_pvc_disk(&vol.name, &clone_name, "20Gi", order);
                            added_disk = true;
                            pvc_notes.push(format!("CDI cloned DataVolume {}", dv.name));
                            pending_dvs.push(clone_name.clone());
                        }
                        Err(e) => pvc_notes.push(format!(
                            "could not clone DataVolume {}: {}",
                            dv.name,
                            sanitize_error(&e)
                        )),
                    }
                }
            }
        }
    }
    if !added_disk {
        builder = builder.add_blank_disk("rootdisk", "20Gi", 1);
    }

    let config = builder.build();
    match client.create_vm(&config).await {
        Ok(_) => {
            let mut ready = Vec::new();
            for dv in &pending_dvs {
                match client
                    .wait_for_data_volume(&namespace, dv, dv_wait_secs())
                    .await
                {
                    Ok(crate::kube::cdi::DataVolumeWait::Ready) => ready.push(dv.clone()),
                    Ok(other) => pvc_notes.push(format!("DataVolume {dv} wait={other:?}")),
                    Err(e) => pvc_notes.push(format!(
                        "DataVolume {dv} wait error: {}",
                        sanitize_error(&e)
                    )),
                }
            }
            let skip_wait = std::env::var("ZORVIA_SKIP_DV_WAIT").ok().as_deref() == Some("1");
            if skip_wait || pending_dvs.is_empty() || ready.len() == pending_dvs.len() {
                let _ = client.start_vm(&namespace, &body.target_name).await;
            } else {
                pvc_notes.push("start deferred: DataVolume import not Succeeded".into());
            }
            Json(json!({
                "name": body.target_name,
                "source": source,
                "pvc_notes": pvc_notes,
                "data_volumes_ready": ready,
                "data_copied": pvc_notes.iter().any(|n| n.contains("CDI")),
                "clone_mode": body.clone_mode.clone().unwrap_or_else(|| "cdi".into()),
                "message": "VM cloned. Start waits for CDI DataVolume Succeeded unless ZORVIA_SKIP_DV_WAIT=1."
            }))
            .into_response()
        }
        Err(e) => {
            let (st, j) = err_json(500, "CLONE_FAILED", &sanitize_error(&e));
            (st, j).into_response()
        }
    }
}

pub async fn fabric_create_snapshot(
    State(state): State<SharedState>,
    Path(vm): Path<String>,
    AxumJson(body): AxumJson<CreateSnapshotBody>,
) -> impl IntoResponse {
    let s = state.read().await;
    let namespace = s.namespace.clone();
    let kube = s.kube_client.client();
    drop(s);

    let snap_name = if body.name.starts_with(&vm) {
        body.name.clone()
    } else {
        format!("{}-{}", vm, body.name)
    };
    let mut cfg = SnapshotConfig::new(&vm, &snap_name);
    if let Some(d) = body.description {
        cfg = cfg.with_description(d);
    }
    let manager = SnapshotManager::from_client(kube, namespace);
    match manager.create_snapshot(&cfg).await {
        Ok(info) => (
            StatusCode::CREATED,
            Json(json!({
                "id": info.name,
                "vm_name": info.vm_name,
                "name": info.name,
                "description": info.description,
                "snapshot_type": body.snapshot_type.unwrap_or_else(|| "Disk".into()),
                "parent_id": null,
                "size_bytes": 0,
                "created": info.created_at.map(|t| t.to_rfc3339()).unwrap_or_default(),
            })),
        )
            .into_response(),
        Err(e) => {
            let (st, j) = err_json(500, "SNAPSHOT_FAILED", &sanitize_error(&e));
            (st, j).into_response()
        }
    }
}

pub async fn fabric_delete_snapshot(
    State(state): State<SharedState>,
    Path((vm, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let _ = vm;
    let s = state.read().await;
    let namespace = s.namespace.clone();
    let kube = s.kube_client.client();
    drop(s);
    let manager = SnapshotManager::from_client(kube, namespace);
    match manager.delete_snapshot(&id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            let (st, j) = err_json(500, "SNAPSHOT_DELETE_FAILED", &sanitize_error(&e));
            (st, j).into_response()
        }
    }
}

pub async fn fabric_revert_snapshot(
    State(state): State<SharedState>,
    Path((vm, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let s = state.read().await;
    let namespace = s.namespace.clone();
    drop(s);
    match crate::snapshots::RestoreManager::new(&namespace).await {
        Ok(rm) => match rm.restore_in_place(&vm, &id).await {
            Ok(_) => StatusCode::NO_CONTENT.into_response(),
            Err(e) => {
                let (st, j) = err_json(500, "REVERT_FAILED", &sanitize_error(&e));
                (st, j).into_response()
            }
        },
        Err(e) => {
            let (st, j) = err_json(500, "REVERT_FAILED", &sanitize_error(&e));
            (st, j).into_response()
        }
    }
}

pub async fn fabric_vm_metrics(
    State(state): State<SharedState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let s = state.read().await;
    let namespace = s.namespace.clone();
    let client = s.client();
    drop(s);

    let vm = client.get_vm(&namespace, &name).await.ok();
    let vmi = client.get_vmi(&namespace, &name).await.ok();
    let paused = if let Some(vmi) = &vmi {
        let phase = vmi.status.as_ref().and_then(|st| st.phase.as_deref());
        let conds: Vec<(&str, &str)> = vmi
            .status
            .as_ref()
            .and_then(|st| st.conditions.as_ref())
            .map(|cs| {
                cs.iter()
                    .map(|c| (c.type_.as_str(), c.status.as_str()))
                    .collect()
            })
            .unwrap_or_default();
        crate::kube::lifecycle::status_is_paused(
            vm.as_ref()
                .and_then(|v| v.status.as_ref())
                .and_then(|st| st.printable_status.as_deref()),
            phase,
            &conds,
        )
    } else {
        false
    };
    let phase = vmi
        .as_ref()
        .and_then(|v| v.status.as_ref())
        .and_then(|st| st.phase.clone())
        .or_else(|| {
            vm.as_ref()
                .and_then(|v| v.status.as_ref())
                .and_then(|st| st.printable_status.clone())
        })
        .unwrap_or_else(|| "Unknown".into());
    let node = vmi
        .as_ref()
        .and_then(|v| v.status.as_ref())
        .and_then(|st| st.node_name.clone());

    let osinfo = client
        .get_vmi_subresource_json(&namespace, &name, "guestosinfo")
        .await
        .ok();
    let fslist = client
        .get_vmi_subresource_json(&namespace, &name, "filesystemlist")
        .await
        .ok();
    let gm =
        crate::kube::guest_metrics::metrics_from_guest_payloads(osinfo.as_ref(), fslist.as_ref());
    let mut source = if gm.agent {
        "guest-agent"
    } else {
        "kubevirt-status"
    };
    let mut cpu_usage = gm.cpu_usage;
    let mut memory_usage = gm.memory_usage;
    let mut disk_usage = gm.disk_usage;
    if let Some((pcpu, pmem, pdisk)) = crate::kube::prom::samples_from_env_file(&name) {
        if pcpu > 0.0 || cpu_usage == 0.0 {
            cpu_usage = pcpu;
        }
        if pmem > 0 {
            memory_usage = pmem;
        }
        if pdisk > 0 {
            disk_usage = pdisk;
        }
        source = "prometheus";
    }
    let point = crate::kube::prom::MetricsPoint {
        ts: chrono::Utc::now().to_rfc3339(),
        cpu_usage,
        memory_usage,
        disk_usage,
    };
    crate::kube::prom::MetricsRing::global().push(&name, point);
    let history = crate::kube::prom::MetricsRing::global().history(&name);
    Json(json!({
        "cpu_usage": cpu_usage,
        "memory_usage": memory_usage,
        "memory_available": gm.memory_available,
        "disk_usage": disk_usage,
        "disk_total": gm.disk_total,
        "network_rx": gm.network_rx,
        "network_tx": gm.network_tx,
        "hostname": gm.hostname,
        "agent": gm.agent,
        "phase": phase,
        "paused": paused,
        "node": node,
        "source": source,
        "history": history,
    }))
}

pub async fn fabric_guest_insight(
    State(state): State<SharedState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let s = state.read().await;
    let namespace = s.namespace.clone();
    let client = s.client();
    drop(s);
    let status = client
        .get_vmi(&namespace, &name)
        .await
        .ok()
        .and_then(|vmi| vmi.status);
    let report =
        crate::guest_insight::GuestInsightReport::from_status(name, namespace, status.as_ref());
    Json(json!(report))
}

fn dv_wait_secs() -> u64 {
    std::env::var("ZORVIA_DV_WAIT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(90)
}

fn guest_wait_secs() -> u64 {
    std::env::var("ZORVIA_GUEST_WAIT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(120)
}

pub async fn fabric_wait_guest_ready(
    State(state): State<SharedState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let s = state.read().await;
    let namespace = s.namespace.clone();
    let client = s.client();
    drop(s);
    match client
        .wait_until_guest_ready(&namespace, &name, guest_wait_secs())
        .await
    {
        Ok(report) if report.cloud_init_ready => Json(json!(report)).into_response(),
        Ok(report) => {
            let (st, j) = err_json(408, "TIMEOUT", &report.reason);
            (st, j).into_response()
        }
        Err(e) => {
            let (st, j) = err_json(500, "WAIT_FAILED", &sanitize_error(&e));
            (st, j).into_response()
        }
    }
}

pub async fn fabric_wait_data_volume(
    State(state): State<SharedState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let s = state.read().await;
    let namespace = s.namespace.clone();
    let client = s.client();
    drop(s);
    match client
        .wait_for_data_volume(&namespace, &name, dv_wait_secs())
        .await
    {
        Ok(crate::kube::cdi::DataVolumeWait::Ready) => Json(json!({
            "name": name,
            "state": "succeeded"
        }))
        .into_response(),
        Ok(crate::kube::cdi::DataVolumeWait::Failed) => {
            let (st, j) = err_json(409, "IMPORT_FAILED", "DataVolume entered Failed phase");
            (st, j).into_response()
        }
        Ok(crate::kube::cdi::DataVolumeWait::Pending) => {
            let (st, j) = err_json(408, "TIMEOUT", "DataVolume did not reach Succeeded in time");
            (st, j).into_response()
        }
        Err(e) => {
            let (st, j) = err_json(500, "WAIT_FAILED", &sanitize_error(&e));
            (st, j).into_response()
        }
    }
}

pub async fn fabric_vm_logs(
    State(state): State<SharedState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    use k8s_openapi::api::core::v1::Pod;
    use kube::api::{Api, ListParams, LogParams};

    let s = state.read().await;
    let namespace = s.namespace.clone();
    let kube_client = s.kube_client.client();
    drop(s);

    let pods: Api<Pod> = Api::namespaced(kube_client, &namespace);
    let lp = ListParams::default().labels(&format!("kubevirt.io/vm={name}"));
    match pods.list(&lp).await {
        Ok(list) => {
            let mut entries = Vec::new();
            for pod in list.items.into_iter().take(3) {
                let Some(pod_name) = pod.metadata.name else {
                    continue;
                };
                let params = LogParams {
                    tail_lines: Some(80),
                    ..Default::default()
                };
                match pods.logs(&pod_name, &params).await {
                    Ok(text) => {
                        for line in text
                            .lines()
                            .rev()
                            .take(80)
                            .collect::<Vec<_>>()
                            .into_iter()
                            .rev()
                        {
                            if !line.is_empty() {
                                entries.push(json!({
                                    "pod": pod_name,
                                    "line": line,
                                }));
                            }
                        }
                    }
                    Err(e) => log::debug!("logs for {pod_name}: {e}"),
                }
            }
            let count = entries.len();
            Json(json!({ "entries": entries, "count": count })).into_response()
        }
        Err(e) => {
            log::warn!("list virt-launcher pods for {name}: {e}");
            Json(json!({ "entries": [], "count": 0 })).into_response()
        }
    }
}

pub async fn fabric_pause_vm(
    State(state): State<SharedState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let s = state.read().await;
    let namespace = s.namespace.clone();
    let client = s.client();
    match client.pause_vm(&namespace, &name).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            // classify_lifecycle_error is the sanitizer here; it needs raw
            // error text (sanitize_error() first would collapse our own
            // ZorviaError::VmNotFound — "VM '...' not found", not prefixed
            // with any of sanitize_error's allowed prefixes — to a generic
            // "Internal server error" before classification ever runs).
            let raw = e.to_string();
            let (code, kind, msg) = crate::kube::lifecycle::classify_lifecycle_error("pause", &raw);
            let (st, j) = err_json(code, kind, &msg);
            (st, j).into_response()
        }
    }
}

pub async fn fabric_resume_vm(
    State(state): State<SharedState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let s = state.read().await;
    let namespace = s.namespace.clone();
    let client = s.client();
    match client.resume_vm(&namespace, &name).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            let raw = e.to_string();
            let (code, kind, msg) =
                crate::kube::lifecycle::classify_lifecycle_error("resume", &raw);
            let (st, j) = err_json(code, kind, &msg);
            (st, j).into_response()
        }
    }
}
