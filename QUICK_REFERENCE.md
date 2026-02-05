# Zorvia Quick Reference Card

## 🎯 Common Commands

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
zorvia delete myvm               # Delete VM
zorvia status myvm               # Detailed status
zorvia clone source target       # Clone VM
```

### Templates
```bash
zorvia templates                 # List all templates
zorvia template ubuntu-22.04     # View template details
```

### Advanced
```bash
zorvia wizard                    # Interactive wizard
zorvia batch config.yaml         # Batch operations
zorvia export myvm               # Export config
zorvia validate config.yaml      # Validate config
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

## 🐧 OS Templates (44 total)

### Linux
- **Ubuntu**: 18.04, 20.04, 22.04, 24.04, latest
- **Fedora**: 38, 39, 40, latest
- **CentOS**: stream9, 7, latest
- **Debian**: 11, 12, latest
- **RHEL**: 8, 9, latest
- **AlmaLinux**: 8, 9, latest
- **Rocky**: 8, 9, latest
- **OpenSUSE**: leap, tumbleweed, latest
- **Alpine**: 3.18, latest
- **Arch**: latest
- **Oracle**: 8, 9, latest

### BSD
- **FreeBSD**: 13, 14, latest

### Container
- **Flatcar**: stable
- **Talos**: latest

### Windows
- 2k19, 2k22, 10, 11, latest

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

## 📚 More Info

- `INNOVATIVE_FEATURES.md` - Complete feature guide
- `OS_TEMPLATES.md` - All OS templates
- `THEME_DESIGN.md` - Theme documentation
- `README.md` - Main documentation
