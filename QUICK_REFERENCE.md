# Zorvia Quick Reference Card

## 🎯 Common Commands

### VM Snapshots & Backup
```bash
zorvia snapshot-create my-vm --name backup-20260205
zorvia snapshot-list my-vm          # List snapshots for VM
zorvia snapshot-list                # List all snapshots
zorvia snapshot-get backup-20260205 # Get snapshot details
zorvia snapshot-restore backup-20260205 --target restored-vm
zorvia snapshot-restore backup-20260205 --in-place  # Overwrite existing
zorvia snapshot-delete old-snapshot
```

### VM Profiles
```bash
zorvia profiles                  # List all profiles
zorvia profiles --details        # Detailed profile info
zorvia profile database          # View specific profile
```

### Multi-VM Blueprints
```bash
zorvia blueprints                # List all blueprints
zorvia blueprints --tag web      # Filter by tag
zorvia blueprint lamp            # View blueprint details
zorvia deploy lamp --dry-run     # Preview deployment
zorvia deploy lamp --prefix prod # Deploy with custom prefix
zorvia deploy lamp --start       # Deploy and start VMs
```

### Resource Recommendations
```bash
zorvia recommend database        # Database workload
zorvia recommend web             # Web server workload
zorvia recommend ml              # Machine learning workload
```

### Health Checks
```bash
zorvia health my-vm              # Check VM health
zorvia health my-vm --detailed   # Detailed checks
```

### VM Operations
```bash
zorvia create myvm --template ubuntu --profile prod
zorvia list                      # List all VMs
zorvia get myvm                  # Get VM details
zorvia start myvm                # Start VM
zorvia stop myvm                 # Stop VM
zorvia pause myvm                # Pause running VMI
zorvia resume myvm               # Resume paused VMI
zorvia terraform-scaffold --output ./terraform/zorvia-vm
zorvia wait-image ubuntu-import --timeout 120
zorvia wait-ready myvm --timeout 120
zorvia delete myvm               # Delete VM
zorvia status myvm               # Detailed status
zorvia clone source target       # Clone VM
```

### Web console & API
```bash
# Lab (HTTPS NodePort 30152)
open https://HOST:30152/app/create
open https://HOST:30152/app/vms/myvm/console

# Auth + list
curl -sk -X POST https://HOST:30152/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"Admin@321"}'
curl -sk -H "Authorization: Bearer $TOKEN" https://HOST:30152/api/vms

# Power
curl -sk -X POST -H "Authorization: Bearer $TOKEN" \
  https://HOST:30152/api/vms/myvm/start
curl -sk -X POST -H "Authorization: Bearer $TOKEN" \
  https://HOST:30152/api/vms/myvm/pause
curl -sk -X POST -H "Authorization: Bearer $TOKEN" \
  https://HOST:30152/api/vms/myvm/resume
```

Serial console: `wss://HOST:30152/ws/console/myvm?token=$TOKEN`  
VNC: `wss://HOST:30152/ws/vnc/myvm?token=$TOKEN`  
SSH: `wss://HOST:30152/ws/ssh/myvm?token=$TOKEN&user=ubuntu`  
Expose: port-forwards API → NodePort (`ZORVIA_EXPOSE_HOST`). See `docs/WEB_CONSOLE.md`.

### Hotplug, resize & migration (web console + API)
```bash
# Hotplug CPU / memory (needs cpu.maxSockets/memory.maxGuest headroom, set by default at create)
curl -sk -X POST -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -d '{"count":4}' https://HOST:30152/api/vms/myvm/hotplug/cpu
curl -sk -X POST -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -d '{"size_mb":1024}' https://HOST:30152/api/vms/myvm/hotplug/memory

# Hotplug disk (bus defaults to scsi — KubeVirt requires it for hotplugged disks)
curl -sk -X POST -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -d '{"path":"my-pvc"}' https://HOST:30152/api/vms/myvm/hotplug/disk

# Resize a PVC-backed disk (grow-only)
curl -sk -X POST -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -d '{"size":"50Gi"}' https://HOST:30152/api/vms/myvm/disks/rootdisk/resize

# Live migration
curl -sk -X POST -H "Authorization: Bearer $TOKEN" https://HOST:30152/api/vms/myvm/migrate
open https://HOST:30152/app/migrations
```
See `docs/WEB_CONSOLE.md` for feature-gate requirements (CPU/memory hotplug needs KubeVirt `VMLiveUpdateFeatures`).

### Rook-Ceph storage (web console + API)
```bash
open https://HOST:30152/app/storage
curl -sk -X POST -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -d '{"name":"mypool","replicated_size":3}' https://HOST:30152/api/storage/rook/pools
curl -sk -X POST -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -d '{"type":"rbd","name":"mysc","pool":"mypool"}' https://HOST:30152/api/storage/rook/storage-classes
```

### Drift, plan & guest insight
```bash
zorvia drift desired.yaml
zorvia plan desired.yaml --vm myvm
zorvia guest-insight myvm
zorvia guest-insight myvm -o json --strict
```

### Templates
```bash
zorvia templates                 # List all templates
zorvia template ubuntu-22.04     # View template details
```

### Kryton (Windows plane)
```bash
# Enable on API: KRYTON_URL, KRYTON_TOKEN, KRYTON_PROJECT
open https://HOST:30152/app/windows
# See docs/KRYTON_INTEGRATION.md
```

### vCenter-style ops
```bash
zorvia inventory                 # DC -> Cluster -> Host/Folder -> VM
zorvia activity                  # Recent tasks / events / alarms
zorvia maintenance-plan NODE
zorvia placement-advisor --cpu 2 --memory-gib 4
# See docs/VCENTER_FEATURE_MATRIX.md
```

### Deploy images (quay.io + golden)
```bash
# Instant: Create VM → pick quay.io/containerdisks/* from GET /api/images
# Golden CDI library (needs StorageClass + CDI):
STORAGE_CLASS=fast ./fixtures/golden-images/generate-bundles.sh
kubectl apply -f fixtures/golden-images/out/
zorvia image-bundle --help
# See docs/GOLDEN_IMAGES.md
```

### Advanced
```bash
zorvia wizard                    # Interactive wizard
zorvia batch config.yaml         # Batch operations
zorvia export myvm               # Export config
zorvia validate config.yaml      # Validate config
zorvia terraform-scaffold --output ./terraform/zorvia-vm
zorvia api-serve --tls --port 5151
```

---

## 📦 Available Profiles

| Profile | CPU | RAM | Disk | Use Case |
|---------|-----|-----|------|----------|
| minimal | 1 | 512Mi | 5Gi | DNS, agents |
| dev | 1 | 2Gi | 10Gi | Development |
| test | 2 | 4Gi | 20Gi | CI/CD |
| web | 4 | 8Gi | 40Gi | Web servers |
| prod | 4 | 8Gi | 40Gi | Production |
| database | 6 | 16Gi | 200Gi | Databases |
| microservice | 2 | 4Gi | 20Gi | Containers |
| high-perf | 8 | 16Gi | 100Gi | ML, big data |

---

## 🏗️ Available Blueprints

| Blueprint | VMs | Description |
|-----------|-----|-------------|
| lamp | 2 | MySQL + Apache |
| k8s-cluster | 3 | K8s control + workers |
| 3tier | 3 | Web + App + DB |
| cicd | 3 | GitLab + Jenkins + Registry |
| dev-stack | 3 | DB + Cache + Workspace |

---

## OS Templates (43 named keys)

### Linux
- **Ubuntu**: 18.04, 20.04, 22.04, 24.04 (`ubuntu` → 22.04)
- **Fedora**: 39, 40, 41 (`fedora` → 41)
- **CentOS Stream**: 8, 9 (`centos` → stream-9)
- **Debian**: 11, 12 (`debian` → 12)
- **RHEL**: 8, 9 (`rhel` → 9)
- **AlmaLinux**: 8, 9 (`almalinux` → 9)
- **Rocky**: 8, 9 (`rocky` → 9)
- **OpenSUSE**: leap, tumbleweed (`opensuse` → leap)
- **Alpine**: 3.19 (`alpine` → 3.19)
- **Arch**: `arch`
- **Oracle**: 8, 9 (`oracle` → 9)

### BSD
- **FreeBSD**: 13, 14 (`freebsd` → 14)

### Container / Kubernetes
- **Flatcar**: `flatcar`
- **Talos**: `talos`

### Windows
- 2019, 2022, 10, 11 (`windows` → 2022)

---

## 🎨 Status Symbols

- ● Green = Running
- ◐ Yellow = Pending/Starting
- ○ Gray = Stopped
- ✗ Red = Failed
- ⟳ Blue = Restarting
- ⏸ Yellow = Paused

---

## 💡 Quick Examples

### Create Development VM
```bash
zorvia create dev-vm --template ubuntu --profile dev
zorvia start dev-vm
```

### Create Production Database
```bash
zorvia recommend database
zorvia create prod-db --template almalinux --profile database
zorvia health prod-db
zorvia start prod-db
```

### Deploy LAMP Stack
```bash
zorvia blueprint lamp
zorvia deploy lamp --prefix myapp --start
zorvia list
```

### Deploy Kubernetes Cluster
```bash
zorvia blueprint k8s-cluster
zorvia deploy k8s-cluster --prefix prod --start
```

### Check VM Health
```bash
zorvia health my-vm --detailed
```

---

## 🔧 Configuration

Theme config: `~/.config/zorvia/tui.toml`

Environment variables:
```bash
export ZORVIA_NAMESPACE=default
export KUBECONFIG=~/.kube/config
```

---

## More Info

- `docs/WEB_CONSOLE.md` - Web UI, Fabric API, console/VNC/SSH, expose
- `docs/KRYTON_INTEGRATION.md` - Windows plane via Kryton
- `docs/TERRAFORM.md` - Terraform scaffold + module
- `docs/DRIFT_GUARD.md` / `docs/CHANGE_PLANNER.md` / `docs/GUEST_INSIGHT.md`
- `docs/GOLDEN_IMAGES.md` - Deploy Linux images (quay containerdisks + CDI golden)
- `docs/INNOVATIVE_FEATURES.md` - Profiles, blueprints, health
- `docs/OS_TEMPLATES.md` - All OS templates
- `docs/THEME.md` - Theme documentation
- `docs/SNAPSHOTS.md` - Snapshot management
- `README.md` - Main documentation
- `CHANGELOG.md` - Release notes
