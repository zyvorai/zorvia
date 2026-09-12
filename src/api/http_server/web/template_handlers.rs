//! Real, curated OS template catalog, backed by `crate::templates::TEMPLATES`
//! -- ~30 pre-built `VMConfig`s (Ubuntu/Fedora/RHEL/Windows/Talos/etc, each
//! with real firmware/feature/clock presets, e.g. Secure Boot UEFI + HyperV
//! enlightenments for Windows) that had zero HTTP route before this.
//!
//! This is a different, real feature from the deleted `Templates.tsx`
//! (which wanted user-authored templates saved from existing VMs, with no
//! backing at all) and from `ContentLibrary.tsx` (vCenter-style versioned
//! libraries, also no backing) -- browse-only for now; translating a
//! template's full `VMConfig` into a `POST /vms` create request is left for
//! a follow-up rather than risked here.

use super::*;
use serde_json::json;

pub async fn list_templates_handler() -> impl IntoResponse {
    Json(json!(crate::templates::TEMPLATES.list_by_family())).into_response()
}

pub async fn get_template_handler(Path(name): Path<String>) -> impl IntoResponse {
    match crate::templates::TEMPLATES.get(&name) {
        Some(config) => Json(config).into_response(),
        None => {
            let (st, j) = err_json(404, "NOT_FOUND", &format!("No template named '{name}'"));
            (st, j).into_response()
        }
    }
}
