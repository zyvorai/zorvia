# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **VM hotplug** — CPU (`POST /api/vms/:name/hotplug/cpu`), memory (`…/hotplug/memory`), disk attach/detach (`…/hotplug/disk[/:id]`), NIC attach/detach (`…/hotplug/nic[/:id]`); web console Hotplug tab. Fabric `POST /api/vms` now sets `cpu.maxSockets`/`memory.maxGuest` headroom by default (4x sockets / 2x memory) so VMs created through the wizard are hotplug-capable without extra steps.
- **Disk resize** — `POST /api/vms/:name/disks/:disk_name/resize` grows a PVC-backed disk in place; `GET /api/vms/:name/disks` lists disks with bus/source/resizable
- **Live migration, for real** — `POST /api/vms/:name/migrate` creates a `VirtualMachineInstanceMigration`; `GET /api/vms/:name/migrations`, `GET/POST /api/migrations/:id[/cancel]`; `/app/migrations` rewritten around KubeVirt's actual migration phases (Pending → Scheduling → PreparingTarget → TargetReady → Running → Succeeded/Failed) — previously real at the kube-client layer but CLI-only, with a UI mockup that never routed
- **VM creation feature parity** — `POST /api/vms` accepts the full `VMConfig` surface: CPU model, dedicated placement, isolate-emulator-thread, memory hugepages, firmware (BIOS/UEFI/secure boot), machine type, TPM/RNG, HyperV/ACPI/APIC features, multiple disks and NICs. Create VM wizard gained a collapsible Advanced Options step
- **Rook-Ceph storage** — `src/rook`: typed CephCluster/CephBlockPool/CephFilesystem/CephObjectStore client, pinned-manifest operator bootstrap, StorageClass + VolumeSnapshotClass provisioning; `/api/storage/rook/*`; `/app/storage` page
- **Kryton in the main wizard** — Create VM's Windows path now renders the live Kryton golden-image catalog and creates through `POST /api/v1/kryton/machines`, instead of requiring a separate flow; see [docs/KRYTON_INTEGRATION.md](docs/KRYTON_INTEGRATION.md)
- **Live metrics graph** — metrics API returns a 30-point `history` ring; `ZORVIA_PROM_SAMPLES` overlays Prometheus text; console shows source
- **Terraform module** — `terraform/modules/zorvia_vm` create/destroy via the Fabric API
- **Guest ready wait** — create+start waits for Ready+IP; `zorvia wait-ready`; `POST /api/vms/:name/wait-ready`
- **Prometheus** — `/api/metrics` exposition, virt-launcher text parser, `deploy/servicemonitor.yaml`; readiness uses `/api/readyz` (kube reachable)
- **Guest-agent metrics** — `GET /api/vms/:name/metrics` reads `guestosinfo` + `filesystemlist`; `GET /api/vms/:name/guest-insight`
- **CDI wait** — clone waits for DataVolume Succeeded before start; `zorvia wait-image`; `POST /api/datavolumes/:name/wait`
- **Terraform provider schema** — scaffold writes `schema.json` (`zorvia_vm`, snapshots, downloads)
- **In-browser SSH** — `/ws/ssh/:vm?user=` proxies `ssh` or `virtctl ssh` into an xterm tab
- **CDI clone** — `POST /api/vms/:name/clone` with `clone_mode=cdi` creates a DataVolume from the source PVC
- **Cloud download jobs** — `POST /api/images/cloud/download` + `GET /api/images/downloads` (SPA-compatible)
- **Terraform scaffold** — `zorvia terraform-scaffold` writes example TF that drives the Fabric API
- **Pause / resume** — CLI (`zorvia pause|resume`), Fabric `POST /api/vms/:name/pause|resume`, and console buttons call KubeVirt VMI `pause` / `unpause` instead of returning 501
- **Template-backed cloud catalog** — `GET /api/images/cloud` lists unique containerdisks from the OS template library
- **Linux expose_vnc** — create-time VNC NodePort is no longer Windows-only
- **PVC-aware clone** — clone allocates empty same-size PVCs for PVC-backed source disks
- **Live-ish metrics/logs** — metrics include phase/paused/node; logs read virt-launcher pods
- **Web console VM ops** — Create VM (Linux cloud-init / Windows), serial console + VNC WebSocket proxies to KubeVirt, NodePort expose (SSH/VNC/RDP), clone, snapshot create/delete/revert; SPA routes under `/app/create` and `/app/vms/:name/console`
- **Fabric-compatible HTTP** — `POST /api/vms`, `/api/images`, port-forwards, cloud-init annotate, clone, snapshots; metrics/logs from KubeVirt; pause/resume via VMI subresources
- **Kube expose helpers** — managed NodePort Services labeled `zorvia.io/vm`; `ZORVIA_EXPOSE_HOST` for UI connection hints
- **Docs** — [docs/WEB_CONSOLE.md](docs/WEB_CONSOLE.md) for UI/API/WebSocket/RBAC
- **Drift Guard** — semantic desired-vs-live VM drift detection with Kubernetes-noise normalization, named-list canonicalization, operational risk scoring, JSON-pointer ignores, CI severity gates, and table/JSON/YAML output
- **Change Planner** — `zorvia plan` turns Drift Guard findings into conservative online/restart/recreate/manual-review execution plans with downtime gates
- **Guest Insight** — `zorvia guest-insight` summarizes QEMU Guest Agent state, guest OS/kernel, interfaces, and readiness score
- **CDI Golden Images** — `zorvia image-bundle` generates versioned DataVolumes plus stable DataSource aliases

### Changed

- **Kubernetes HTTPS (Veyron-style)** — in-pod rustls TLS via openssl init container; Service NodePort **30152** (`https://HOST:30152`). Host systemd `zorvia-web` is no longer the lab front door.
- **SPA Core nav** — Dashboard, VMs, Create VM, Favorites, Snapshots; product branding **Zorvia**
- **Renamed project to Zorvia** — crate, binary, config paths, deploy manifests, and docs now use `zorvia` / `Zorvia` (repository: [zyvorai/zorvia](https://github.com/zyvorai/zorvia))
- **Apache License 2.0 only** — removed the MIT dual-license; `LICENSE` is Apache-2.0 exclusively

### Security

- **Path traversal prevention** - Profile and blueprint storage now sanitize names to block directory traversal attacks
- **CORS restricted by default** - API server CORS defaults to disabled instead of wildcard `*` origins
- **RDP cert validation enforced** - `ignore_cert` field is now ignored; TLS certificate validation is always enforced
- **Error message sanitization** - HTTP API responses no longer leak internal Kubernetes error details to clients
- **Request ID uniqueness** - API request IDs now include random suffix to prevent collisions under concurrency
- **PAM username length** - Username validation tightened from 256 to 32 characters (PAM LOGIN_NAME_MAX)
- **Console WS auth** — in-cluster proxy uses SA `token_file` + cluster CA when dialing KubeVirt subresources

### Fixed

#### Error Messages Hidden Behind Generic 500s
Found while browser-testing VM creation and day-2 ops end-to-end against a real cluster: several handlers ran the generic `sanitize_error()` *before* their own classification logic, which collapsed Zorvia's own safe, structured error messages (e.g. `ZorviaError::VmExists`, `"VOLUME_NOT_FOUND: …"`, `"SHRINK_NOT_SUPPORTED: …"` — none start with `sanitize_error`'s allowed prefixes) down to a bare "Internal server error" before the classification checks ever saw the real text — so the classification always fell through to a generic 500.
- **VM create name conflicts** — `POST /api/vms` now reports `409` with the real "VM 'x' already exists" message instead of a generic 500
- **Disk resize failures** — `POST /api/vms/:name/disks/:name/resize` now reports `404`/`501`/`409` with the real reason instead of a generic 500
- **`sanitize_error()` itself** — cut the "keep the first sentence" boundary at the first `": "`, which for kube-rs's `"ApiError: <reason>"` format meant *any* passthrough error was truncated to the literal word `"ApiError"` with zero detail; now cuts at sentence-end (`". "`), bounded to 500 chars
- **NIC hotplug 404s** — `addinterface`/`removeinterface` unconditionally turned any 404 into "VM not found", including the case where the subresource route itself isn't registered on the cluster's KubeVirt version — now reports `501 UNSUPPORTED` for that case instead of falsely claiming a running VM doesn't exist
- **vCPU count after hotplug** — every VM API response (list/get/hotplug) computed vCPU count from `domain.cpu.cores` alone; CPU hotplug raises `sockets`, not `cores`, so the reported count silently reverted to the pre-hotplug value forever after a successful hotplug. Now `cores * sockets * threads`

#### Golden Image Downloads Were a No-Op
`DownloadRegistry::start()` built the CDI DataVolume/DataSource manifests and reported "Completed" without ever calling the Kubernetes API — a VM "created" from a downloaded image would fail since nothing was ever imported. Now actually applies both objects; the `dv:`-prefixed image reference it previously returned (which nothing recognized) is now `datavolume:<name>`, handled by the Create VM disk-image parser. Also added an explicit `accessModes: [ReadWriteOnce]` to the DataVolume spec — CDI rejects the import with `ErrClaimNotValid` on any StorageClass without a StorageProfile access mode (e.g. k3s's built-in `local-path`).

#### Crash Prevention
- **Terminal cleanup on panic** - TUI now restores terminal state even if `app.run()` panics or errors
- **Remove server panics** - Replaced `.unwrap()` with safe fallbacks in HTTP server JSON serialization and K8s client initialization
- **Lock poisoning recovery** - TUI blueprint/profile views use `unwrap_or_else` instead of `.expect()` on RwLock
- **Storage init fallback** - Profile/blueprint storage falls back to read-only mode instead of panicking on init failure
- **Rollback lifetime safety** - `execute_rollback()` returns owned `RollbackExecution` instead of borrowed reference

#### Error Handling
- **K8s connection failures surfaced** - Replaced `.unwrap_or_default()` with proper `?` error propagation on `list_vms()` calls in handlers and HTTP server
- **Evacuation error reporting** - `list_all_vms()` in backup handler now logs warnings on failure instead of silently returning empty
- **Audit log overflow warning** - Audit log now emits `log::warn!` when dropping oldest events at capacity
- **Buffer trim logging** - Anomaly detector, autoscaler, leak detector, and log aggregator now log when trimming history buffers

#### Arithmetic Safety
- **Integer overflow prevention** - Cost handler casts use `(val as u64).min(u32::MAX as u64) as u32` for memory/storage values
- **Pagination precision** - Page count calculation uses `u64` arithmetic before clamping to `u32` to avoid truncation
- **Year overflow** - Cost report `monthly_report()` uses `year.saturating_add(1)` instead of `year + 1`
- **Division by zero** - Cost forecast `project_weighted()` now guards `recent_days > 0.0` before dividing

#### Drain Panic Prevention
- Fixed 10 `Vec::drain()` operations across `audit_trail`, `search_history`, `tui/state`, `autoscaler`, `anomaly`, `log_aggregation`, `custom_metrics`, `leak_detector`, and `notifications` that could panic on boundary conditions

#### Logic Bugs
- **Anti-affinity rule fix** - Empty `vm_selector` now correctly means "no match" instead of unconditionally matching all nodes with VMs
- **Cron range validation** - Invalid ranges like `"5-1"` now return `false` instead of silently misbehaving
- **String slice bounds check** - Log pattern extraction guards against out-of-bounds string slicing on trailing quote characters

#### TUI Fixes
- **Tab state preserved** - VM details view retains selected tab when switching views (was always reset to 0)
- **Widget rendering bounds** - Input widget help text uses `saturating_add/sub` to prevent rendering outside allocated area
- **Placement bounds check** - Placement engine uses `.get(i)` instead of direct `[i]` indexing for node alternatives
- **Filter index validation** - `cycle_status_filter()` resets selection index safely against filtered list bounds
- **IP lookup error logging** - TUI state refresh logs debug message on `get_vm_ip()` failure instead of silently dropping
- **Unused import removed** - Removed unused `Span` import from bar chart widget (eliminated compiler warning)

#### Connection Pooling
- **HTTP server client reuse** - All 8 API handlers refactored to use shared `WebState.get_client()` instead of creating new `KubeClient::new()` per request

#### RDP Session
- **Serialization error handling** - RDP session creation logs error and returns error JSON instead of silently returning empty object

## [0.2.0] - 2026-02-28

### Added

#### Application Configuration
- Layered config file support: `/etc/zorvia/config.toml` (system) + `~/.config/zorvia/config.toml` (user)
- `config-show` command - display active configuration with source indicators
- `config-init` command - generate default config file
- Configurable: namespace, kubeconfig, API port/host/TLS/auth, logging level, output format, TUI preferences
- CLI args always take priority over config file values

#### Storage Module
- PVC management types: `PvcSpec`, `PvcStatus`, `StorageClassInfo`
- Storage size utilities: `parse_size_to_bytes()`, `format_bytes()` for K8s size strings (Ki/Mi/Gi/Ti)
- Builder pattern for PVC specs with namespace, storage class, access modes, volume mode

#### TUI Improvements
- Implemented actual VM creation in interactive mode (was stub)
- Implemented actual snapshot creation in interactive mode (was stub)
- Extract disk info from KubeVirt volumes instead of hardcoded values
- Extract node info from VM status conditions

#### CLI Tests
- 25 new CLI argument parsing tests
- Tests for core commands, resource overrides, flags, error cases

#### Integration Tests
- 15 new integration tests (6 → 21 total)
- All 44 templates validated and converted to KubeVirt
- All profiles and blueprints validated
- Example file parsing tests
- End-to-end workflow tests

### Changed

#### Architecture Refactor
- Extracted all command handlers from `lib.rs` into `src/handlers/` module (11 submodules)
- `lib.rs` reduced from 5,335 to 575 lines (-89%)
- Handler modules: vm, profiles, infra, backup, security, cost, automation, observability, multitenancy, devexp, api

#### Code Quality
- Eliminated all 79 compiler warnings
- Eliminated all 64 clippy warnings
- Replaced ~30 unsafe `.unwrap()` calls with proper error handling
- Fixed RwLock guards held across await points
- Renamed `from_str()` methods to `parse()` to avoid `FromStr` trait confusion
- Applied `cargo fmt` across entire codebase (189 files)

#### Naming Conventions
- Fixed non-camel-case enum variants: `PCI_DSS` → `PciDss`, `AI_ML` → `AiMl`, `AWS_KMS` → `AwsKms`, etc.
- Fixed deprecated `Frame::size()` → `Frame::area()`

#### CI/CD
- Added `RUST_MIN_STACK` to prevent test stack overflow
- Added job dependencies: fmt → clippy → test → build
- Consolidated cache paths

#### Documentation
- Updated `DEVELOPMENT.md` to reflect current state
- Removed 11 stale progress/completion reports
- Moved feature docs into `docs/` directory
- Clean project root: README, DEVELOPMENT, CONTRIBUTING, CHANGELOG, SECURITY, QUICK_REFERENCE

### Fixed
- Fixed 2 failing tests (`test_create_custom` in profiles and blueprints) - stale test data cleanup
- Fixed 5 snapshot test compilation errors (async/await, type mismatches)
- Fixed test imports broken by unused import cleanup
- Fixed 17 unused `mut` warnings in test code

#### Release Build
- Added LTO, single codegen unit, strip symbols to release profile
- Binary size reduced from 17MB to 11MB

### Statistics
- Commands: 147 (145 + config-show + config-init)
- Templates: 44 OS templates
- Resource Profiles: 8 built-in
- Deployment Blueprints: 5 built-in
- Tests: 2,076 (all passing)
- Compiler warnings: 0
- Clippy warnings: 0
- Lines of code: ~76,400

## [0.1.0] - 2024-02-05

### Added

#### Core VM Management
- `create` command - Create VMs from templates or configuration files
- `list` command - List VMs with table, YAML, or JSON output
- `get` command - Get detailed VM information
- `delete` command - Delete VMs with confirmation prompt
- `start`, `stop`, `restart` commands - VM lifecycle management
- `status` command - Detailed VM status with watch mode
- `clone` command - Clone existing VMs
- `resources` command - Cluster-wide resource usage summary
- `export` command - Export VM configurations
- `wizard` command - Interactive VM creation
- `batch` command - Batch VM creation from files

#### Templates & Configuration
- 6 built-in VM templates (Ubuntu, CentOS, Fedora, Debian, RHEL, Windows)
- `generate` command - Generate VM manifests
- `validate` command - Validate configuration files
- Cloud-init support, Builder pattern

#### Kubernetes Integration
- Full KubeVirt VirtualMachine CRD support
- VMConfig to KubeVirt manifest converter
- CRUD operations via Kubernetes API

#### Developer Features
- 31 unit and integration tests
- Library API for programmatic usage
- CI/CD with GitHub Actions

[Unreleased]: https://github.com/zyvorai/zorvia/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/zyvorai/zorvia/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/zyvorai/zorvia/releases/tag/v0.1.0
