//! Passthrough-device inventory and preflight (see `crate::devices`).
//!
//! `GET  /api/v1/devices`            what each node advertises, what KubeVirt permits (cluster.admin)
//! `POST /api/v1/devices/preflight`  would these devices / SR-IOV networks start? (vm.create)
//! VM creation runs the same preflight and refuses (422) a VM that cannot be scheduled.

use super::*;
use crate::config::HostDeviceConfig;
use crate::devices::{
    has_errors, preflight, DeviceInventory, Issue, KubevirtDevices, NodeDevices, SriovNetwork,
};
use k8s_openapi::api::core::v1::Node;
use kube::api::{Api, ListParams};
use kube::core::{ApiResource, DynamicObject, GroupVersionKind};
use std::collections::BTreeMap;

const NAD_RESOURCE_ANNOTATION: &str = "k8s.v1.cni.cncf.io/resourceName";

/// A NetworkAttachmentDefinition lookup result.
enum Nad {
    /// The CRD is not installed (no Multus).
    NoCrd,
    NotFound,
    Found(Option<String>),
}

async fn lookup_nad(client: &kube::Client, namespace: &str, name: &str) -> Nad {
    let api = crate::migration_import::networks::attachment_api(client, namespace);
    match api.get(name).await {
        Ok(nad) => Nad::Found(
            nad.metadata
                .annotations
                .and_then(|a| a.get(NAD_RESOURCE_ANNOTATION).cloned()),
        ),
        // 404 for the *kind* (CRD absent) says "could not find the requested resource";
        // 404 for the *object* names it.
        Err(kube::Error::Api(e))
            if e.code == 404 && e.message.contains("could not find the requested resource") =>
        {
            Nad::NoCrd
        }
        Err(kube::Error::Api(e)) if e.code == 404 => Nad::NotFound,
        // Cannot tell (RBAC, transient): report as not found rather than guess installed.
        Err(_) => Nad::NotFound,
    }
}

pub(crate) async fn collect_inventory(client: &kube::Client) -> DeviceInventory {
    let mut inv = DeviceInventory::default();
    let nodes: Api<Node> = Api::all(client.clone());
    if let Ok(list) = nodes.list(&ListParams::default()).await {
        for n in list.items {
            let alloc: BTreeMap<String, String> = n
                .status
                .and_then(|s| s.allocatable)
                .map(|a| a.into_iter().map(|(k, v)| (k, v.0)).collect())
                .unwrap_or_default();
            inv.nodes.push(NodeDevices::from_allocatable(
                n.metadata.name.as_deref().unwrap_or(""),
                &alloc,
            ));
        }
    }
    let ar = ApiResource::from_gvk(&GroupVersionKind::gvk("kubevirt.io", "v1", "KubeVirt"));
    let kv: Api<DynamicObject> = Api::all_with(client.clone(), &ar);
    if let Ok(list) = kv.list(&ListParams::default()).await {
        if let Some(cr) = list.items.first() {
            let cfg = cr
                .data
                .pointer("/spec/configuration")
                .cloned()
                .unwrap_or_default();
            inv.kubevirt = Some(KubevirtDevices::from_configuration(&cfg));
        }
    }
    inv.multus_installed = !matches!(
        lookup_nad(client, "default", "zorvia-probe").await,
        Nad::NoCrd
    );
    inv
}

async fn resolve_sriov(
    client: &kube::Client,
    namespace: &str,
    names: &[String],
) -> Vec<SriovNetwork> {
    let mut out = Vec::new();
    for n in names {
        let (exists, resource_name) = match lookup_nad(client, namespace, n).await {
            Nad::Found(r) => (true, r),
            _ => (false, None),
        };
        out.push(SriovNetwork {
            name: n.clone(),
            exists,
            resource_name,
        });
    }
    out
}

/// Names of the SR-IOV networks an interface list asks for.
pub(crate) fn sriov_names(interfaces: &[crate::config::InterfaceConfig]) -> Vec<String> {
    interfaces
        .iter()
        .filter_map(|i| match &i.network_type {
            crate::config::NetworkType::SRIOV { name } => Some(name.clone()),
            _ => None,
        })
        .collect()
}

/// Preflight for VM creation: only touches the cluster when something is requested.
pub(crate) async fn preflight_for_create(
    client: &kube::Client,
    namespace: &str,
    devices: &[HostDeviceConfig],
    sriov: &[String],
) -> Vec<Issue> {
    if devices.is_empty() && sriov.is_empty() {
        return Vec::new();
    }
    let inv = collect_inventory(client).await;
    let nets = resolve_sriov(client, namespace, sriov).await;
    preflight(devices, &nets, &inv)
}

pub async fn devices_inventory_handler(State(state): State<SharedState>) -> impl IntoResponse {
    let client = state.read().await.client().client();
    Json(collect_inventory(&client).await).into_response()
}

#[derive(Debug, Deserialize)]
pub struct PreflightBody {
    #[serde(default)]
    pub devices: Vec<HostDeviceConfig>,
    /// Names of SR-IOV NetworkAttachmentDefinitions in `namespace`.
    #[serde(default)]
    pub sriov_networks: Vec<String>,
    #[serde(default)]
    pub namespace: Option<String>,
}

pub async fn devices_preflight_handler(
    State(state): State<SharedState>,
    Json(body): Json<PreflightBody>,
) -> impl IntoResponse {
    for d in &body.devices {
        if let Err(e) = d.validate() {
            let (st, j) = err_json(400, "INVALID", &e);
            return (st, j).into_response();
        }
    }
    let s = state.read().await;
    let default_ns = s.namespace.clone();
    let client = s.client().client();
    drop(s);
    let ns = body.namespace.unwrap_or(default_ns);
    let issues = preflight_for_create(&client, &ns, &body.devices, &body.sriov_networks).await;
    Json(serde_json::json!({ "ok": !has_errors(&issues), "issues": issues })).into_response()
}
