//! Image catalogs derived from the built-in OS template library.

use crate::config::DiskSource;
use crate::templates::TEMPLATES;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CloudImage {
    pub name: String,
    pub path: String,
    pub format: String,
    pub template: String,
    pub size_bytes: u64,
}

/// Unique containerdisk images referenced by OS templates.
pub fn cloud_images_from_templates() -> Vec<CloudImage> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for template_name in TEMPLATES.list() {
        let Some(cfg) = TEMPLATES.get(&template_name) else {
            continue;
        };
        for disk in &cfg.disks {
            if let DiskSource::ContainerDisk { image } = &disk.source {
                if seen.insert(image.clone()) {
                    out.push(CloudImage {
                        name: format!("{} (template {template_name})", image),
                        path: image.clone(),
                        format: "containerdisk".into(),
                        template: template_name.clone(),
                        size_bytes: 0,
                    });
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_non_empty_and_unique() {
        let images = cloud_images_from_templates();
        assert!(
            images.len() >= 10,
            "expected a real catalog, got {}",
            images.len()
        );
        let mut paths: Vec<_> = images.iter().map(|i| i.path.as_str()).collect();
        let before = paths.len();
        paths.sort();
        paths.dedup();
        assert_eq!(before, paths.len(), "duplicate image paths in catalog");
        assert!(images.iter().any(|i| i.path.contains("ubuntu")));
        assert!(images.iter().any(|i| i.path.contains("fedora")));
    }
}
