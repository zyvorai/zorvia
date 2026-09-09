//! vCenter-style operational primitives implemented with Kubernetes/KubeVirt semantics.
//! These modules do not create a second control-plane database: inventory hierarchy,
//! metadata, activity and maintenance are derived from or stored on native resources.

pub mod activity;
pub mod inventory;
pub mod maintenance;
pub mod metadata;
