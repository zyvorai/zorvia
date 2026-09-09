//! Image catalogs derived from the built-in OS template library.

use crate::config::DiskSource;
use crate::templates::TEMPLATES;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CloudImage {
    pub name: String,
    pub distro: String,
    pub version: String,
    pub url: String,
    pub format: String,
    pub arch: String,
    pub path: String,
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
                    let (distro, version) = split_image(image);
                    out.push(CloudImage {
                        name: format!("{distro} {version}"),
                        distro,
                        version,
                        url: image.clone(),
                        format: "containerdisk".into(),
                        arch: "amd64".into(),
                        path: image.clone(),
                        template: template_name.clone(),
                        size_bytes: 0,
                    });
                }
            }
        }
    }
    out
}

fn split_image(image: &str) -> (String, String) {
    let rest = image.rsplit('/').next().unwrap_or(image);
    match rest.split_once(':') {
        Some((distro, version)) => (distro.to_string(), version.to_string()),
        None => (rest.to_string(), "latest".into()),
    }
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
        assert!(images.iter().any(|i| i.distro.contains("ubuntu") || i.path.contains("ubuntu")));
        assert!(images.iter().any(|i| !i.url.is_empty()));
    }
}
