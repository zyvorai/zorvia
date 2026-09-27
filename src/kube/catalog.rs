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

/// One entry in the Create VM / Disk Images catalog (`GET /api/images`, `zorvia images`).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DiskImage {
    pub name: String,
    pub path: String,
    pub format: String,
    pub size_bytes: u64,
}

const GIB: u64 = 1024 * 1024 * 1024;

const CONTAINERDISKS: &[(&str, &str)] = &[
    ("Ubuntu 24.04", "ubuntu:24.04"),
    ("Ubuntu 22.04", "ubuntu:22.04"),
    ("Ubuntu 20.04", "ubuntu:20.04"),
    ("Ubuntu 18.04", "ubuntu:18.04"),
    ("Fedora 41", "fedora:41"),
    ("Fedora 40", "fedora:40"),
    ("Fedora 39", "fedora:39"),
    ("CentOS Stream 9", "centos-stream:9"),
    ("CentOS Stream 8", "centos-stream:8"),
    ("Debian 12", "debian:12"),
    ("Debian 11", "debian:11"),
    ("AlmaLinux 9", "almalinux:9"),
    ("AlmaLinux 8", "almalinux:8"),
    ("Rocky Linux 9", "rockylinux:9"),
    ("Rocky Linux 8", "rockylinux:8"),
    ("Alpine 3.19", "alpine:3.19"),
];

/// Blank disks plus ready quay.io/containerdisks images (major Linux).
/// Containerdisk size is unknown until pulled, so it is reported as 0.
pub fn disk_image_catalog() -> Vec<DiskImage> {
    let blank = [20u64, 40].into_iter().map(|gi| DiskImage {
        name: format!("Blank disk ({gi} Gi)"),
        path: format!("blank:{gi}Gi"),
        format: "blank".into(),
        size_bytes: gi * GIB,
    });
    let containerdisks = CONTAINERDISKS.iter().map(|(name, image)| DiskImage {
        name: format!("{name} (containerdisk)"),
        path: format!("quay.io/containerdisks/{image}"),
        format: "containerdisk".into(),
        size_bytes: 0,
    });
    blank.chain(containerdisks).collect()
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
        assert!(images
            .iter()
            .any(|i| i.distro.contains("ubuntu") || i.path.contains("ubuntu")));
        assert!(images.iter().any(|i| !i.url.is_empty()));
    }

    #[test]
    fn disk_image_catalog_matches_api_shape() {
        let images = disk_image_catalog();
        assert_eq!(images.len(), 18);
        assert_eq!(images[0].path, "blank:20Gi");
        assert_eq!(images[0].size_bytes, 21_474_836_480);
        assert_eq!(images[1].size_bytes, 42_949_672_960);
        assert_eq!(images[2].name, "Ubuntu 24.04 (containerdisk)");
        assert_eq!(images[2].path, "quay.io/containerdisks/ubuntu:24.04");
        assert!(images[2..]
            .iter()
            .all(|i| i.format == "containerdisk" && i.size_bytes == 0));
    }
}
