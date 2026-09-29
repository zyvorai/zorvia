# Leaving OpenShift Virtualization

OpenShift Virtualization runs KubeVirt. Its VMs are ordinary KubeVirt `VirtualMachine` objects. So moving off OpenShift's control plane does not mean rewriting your VMs. It means choosing a different platform around the same objects.

Zorvia and ZeusOS are two ways to do that. This page compares them with OpenShift, including where OpenShift is the better choice.

> Every Zorvia "Yes" below maps to a feature in [FEATURE_MATURITY.md](FEATURE_MATURITY.md). Only **GA** and **Beta** items are production promises. Anything **Experimental** is labelled as such.

## Two doors

| | **Zorvia** | **ZeusOS Enterprise** |
|---|---|---|
| Licence | Apache-2.0, free for production | Commercial. Trial, then licence via [sales@zyvor.dev](mailto:sales@zyvor.dev) |
| Best for | Teams that run KubeVirt themselves and want a real control plane without a subscription | Teams that want vendor support, multi-cluster, incident tooling and help moving workloads |
| Surfaces | CLI, TUI, web console, one Fabric API | Web, TUI, CLI and API |
| Support | Community (GitHub issues) | Vendor support under your licence |
| Start | [Quick start](../README.md#quick-start) | [zyvor.dev/zeus-os](https://zyvor.dev/zeus-os) (30-day trial, no sign-up) |

**Start with Zorvia** to prove the model against your own cluster at no cost. **Move to Enterprise** when you need support you can put in a contract, more than one cluster under one roof, or a partner to help with the move.

Editions listed on the ZeusOS page are Community (single cluster), Professional (multi-cluster), Enterprise (AI incident center) and Service Provider (multi-tenant). Pricing is by quote.

## Honest comparison

| Capability | OpenShift Virtualization | Zorvia | ZeusOS Enterprise |
|---|---|---|---|
| VM create and day-2 without hand-written YAML | Yes | Yes (`vm-lifecycle`, GA) | Yes |
| Live migration between nodes | Yes | Yes (`live-migration`, GA; needs shared storage) | Yes |
| Snapshots and restore | Yes | Yes (`snapshots-restore`, GA) | Yes |
| Web console with VNC and serial | Yes | Yes (`web-console`, GA) | Yes |
| Terminal UI | No | Yes | Yes |
| Audit trail with export | Cluster audit log | Yes (`audit-trail`, GA, JSONL export) | Yes |
| SSO | Built-in OAuth server | OIDC (`oidc`, Beta) | See ZeusOS docs |
| Drift detection and change-plan gating | No | Yes (`zorvia change drift` / `zorvia change plan`) | Yes |
| Runs on any KubeVirt cluster | No, OpenShift only | Yes | Yes |
| Licence cost | Red Hat subscription | $0 | Commercial licence |
| Vendor support contract | Yes (Red Hat) | No, community | Yes |
| Multi-cluster management | Yes (Advanced Cluster Management) | Experimental (`fleet-multicluster`, inventory stub) | Yes (Professional and up) |
| VM migration from VMware | Yes (Migration Toolkit for Virtualization) | Experimental (`transiva-migration`, plan only) | Ask sales; migration tooling sits on the Enterprise side |
| Operator lifecycle and certified ecosystem | Yes (OLM, certified operators) | No | No |
| Telco-grade networking (SR-IOV, NFV, encrypted mesh) | Mature | Experimental (`gpu-sriov-numa`, plan only) | Ask sales |

## Choose OpenShift when

- You need a Red Hat support contract or certified-stack requirement written into a procurement rule.
- You already run OpenShift for containers and want one platform, one vendor.
- You depend on operators from the certified catalogue, or on telco-grade networking today.
- You need cluster-to-cluster VM migration tooling that is already GA.

## Choose Zorvia when

- You want KubeVirt without a per-node subscription and are happy to run it yourself.
- You want the same actions from a terminal, a TUI and a browser.
- You want drift and change-plan checks before a change ships.

## Choose ZeusOS Enterprise when

- You want the same KubeVirt-native approach with a vendor behind it.
- You run several clusters or need help planning a migration.
- You want a partner to be accountable for the move.

## What does not change

Your `VirtualMachine`, `DataVolume` and `PersistentVolumeClaim` objects stay as they are. The [adoption runbook](adopt-existing-kubevirt-cluster.md) shows how to point Zorvia at an existing KubeVirt cluster, read-only first.

## What is not done yet

- Zorvia has no production VM importer. Transiva (VMware to KubeVirt) is Experimental and plan-only. Do not plan a migration around it.
- Running Zorvia on OpenShift itself has not been verified. See the runbook for what to check.
- Format conversion (VMDK, VHD to qcow2) is not shipped.

## Cost

Red Hat does not publish a list price for OpenShift Virtualization. It states that OpenShift Virtualization Engine is licensed per bare-metal node ([Red Hat](https://www.redhat.com/en/technologies/cloud-computing/openshift/virtualization-engine)). Put your own quote against the licence line above. A calculator that takes your figure, and assumes none, is planned.

## Talk to us

- **Try Enterprise:** [zyvor.dev/zeus-os](https://zyvor.dev/zeus-os)
- **Talk to sales:** [sales@zyvor.dev](mailto:sales@zyvor.dev). Tell us your node count and your current subscription, and we will tell you what is possible today.
