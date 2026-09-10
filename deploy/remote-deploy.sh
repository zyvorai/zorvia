#!/bin/bash
# ============================================================================
# remote-deploy.sh — Full zorvia deployment to a remote server
# ============================================================================
# Rsync source to remote, compile there, install binary + service, verify.
#
# Usage:
#   ./deploy/remote-deploy.sh <host> [user] [password]
#   ./deploy/remote-deploy.sh 185.165.240.5 sus
#   ./deploy/remote-deploy.sh 185.165.240.5 sus mypassword
#   ./deploy/remote-deploy.sh 185.165.240.5 sus --quick    # skip deps
#   ./deploy/remote-deploy.sh 185.165.240.5 sus --uninstall
#
# Environment variables:
#   DEPLOY_HOST=185.165.240.5
#   DEPLOY_USER=sus
#   DEPLOY_PASS=mypassword
#   DEPLOY_DIR=/home/sus/zorvia  (auto-detected from login user)
# ============================================================================

set -euo pipefail

info()  { echo "  [ok] $*"; }
warn()  { echo "  [!!] $*"; }
error() { echo "  [ERR] $*"; exit 1; }
step()  { echo ""; echo "  --- $*"; }

# ── Parse args ──
QUICK_MODE=false
UNINSTALL_MODE=false
POSITIONAL=()
for arg in "$@"; do
    case "$arg" in
        --quick)     QUICK_MODE=true ;;
        --uninstall) UNINSTALL_MODE=true ;;
        --help|-h)
            echo "Usage: $0 <host> [user] [password] [--quick|--uninstall]"
            echo ""
            echo "  --quick      Skip Rust toolchain install (rsync + build only)"
            echo "  --uninstall  Remove zorvia from remote server"
            echo ""
            echo "Full mode installs: Rust toolchain, builds from source,"
            echo "deploys binary + systemd service + k3s manifest."
            exit 0
            ;;
        *)  POSITIONAL+=("$arg") ;;
    esac
done

HOST="${POSITIONAL[0]:-${DEPLOY_HOST:-}}"
USER="${POSITIONAL[1]:-${DEPLOY_USER:-sus}}"
PASS="${POSITIONAL[2]:-${DEPLOY_PASS:-}}"

[ -z "$HOST" ] && error "Usage: $0 <host> [user] [password] [--quick]"

# Determine remote home directory based on login user
if [ "$USER" = "root" ]; then
    REMOTE_HOME="/root"
else
    REMOTE_HOME="/home/${USER}"
fi
REMOTE_DIR="${DEPLOY_DIR:-${REMOTE_HOME}/zorvia}"

# Use sudo when not deploying as root
SUDO=""
[ "$USER" != "root" ] && SUDO="sudo"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

[ -f "$REPO_DIR/Cargo.toml" ] || error "Not in zorvia repo: $REPO_DIR"

# ── SSH/rsync wrappers ──
_ssh() {
    if [ -n "$PASS" ]; then
        SSHPASS="$PASS" sshpass -e ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "$@"
    else
        ssh -o StrictHostKeyChecking=no "${USER}@${HOST}" "$@"
    fi
}

_scp() {
    if [ -n "$PASS" ]; then
        SSHPASS="$PASS" sshpass -e scp -o StrictHostKeyChecking=no "$@"
    else
        scp -o StrictHostKeyChecking=no "$@"
    fi
}

_rsync() {
    local ssh_cmd="ssh -o StrictHostKeyChecking=no"
    if [ -n "$PASS" ]; then
        ssh_cmd="sshpass -e $ssh_cmd"
    fi
    SSHPASS="$PASS" rsync -avz \
        --exclude='target/' \
        --exclude='.git' \
        --exclude='web/node_modules/' \
        --exclude='web/coverage/' \
        --exclude='*.qcow2' --exclude='*.vmdk' --exclude='*.raw' \
        --exclude='*.iso' --exclude='*.img' \
        -e "$ssh_cmd" \
        "$@"
}

# ── Preflight ──
if [ -n "$PASS" ] && ! command -v sshpass &>/dev/null; then
    error "sshpass required for password auth. Install: dnf install sshpass"
fi

# ── Uninstall mode ──
if $UNINSTALL_MODE; then
    echo ""
    echo "  ============================================"
    echo "    zorvia Remote Uninstall"
    echo "  ============================================"
    echo ""
    echo "  Host: ${USER}@${HOST}"
    echo ""

    step "Uninstalling zorvia"
    _ssh "
        $SUDO systemctl stop zorvia-web.service 2>/dev/null || true
        $SUDO systemctl disable zorvia-web.service 2>/dev/null || true
        $SUDO rm -f /usr/local/bin/zorvia
        $SUDO rm -f /etc/systemd/system/zorvia-web.service
        $SUDO systemctl daemon-reload
        rm -rf $REMOTE_DIR
        echo 'Done'
    " 2>&1
    info "zorvia removed from ${HOST}"
    echo ""
    exit 0
fi

TOTAL_STEPS=5
$QUICK_MODE && TOTAL_STEPS=3

echo ""
echo "  ============================================"
echo "    zorvia Remote Deployment"
echo "  ============================================"
echo ""
echo "  Host:     ${USER}@${HOST}"
echo "  Auth:     $([ -n "$PASS" ] && echo "password" || echo "SSH key")"
echo "  Local:    $REPO_DIR"
echo "  Remote:   $REMOTE_DIR"
echo "  Mode:     $($QUICK_MODE && echo "quick (rsync + build only)" || echo "full (Rust toolchain + build + deploy)")"
echo ""

# ── Step 1: Rsync repo ──
step "Step 1/${TOTAL_STEPS}: Syncing repository to ${HOST}"

if [ -f "$REPO_DIR/web/package.json" ]; then
    if [ ! -f "$REPO_DIR/web/dist/index.html" ]; then
        echo "  Building web UI (npm run build)…"
        (cd "$REPO_DIR/web" && npm ci --legacy-peer-deps >/dev/null 2>&1 || npm install --legacy-peer-deps >/dev/null 2>&1) \
          && (cd "$REPO_DIR/web" && npm run build) \
          || warn "web build failed; deploy may serve placeholder UI"
    else
        echo "  web/dist present — skipping local npm build"
    fi
fi

_rsync "$REPO_DIR/" "${USER}@${HOST}:${REMOTE_DIR}/" 2>&1 | tail -3
info "Synced to ${HOST}:${REMOTE_DIR}"

if ! $QUICK_MODE; then
    # ── Step 2: Install Rust toolchain if needed ──
    step "Step 2/${TOTAL_STEPS}: Ensuring Rust toolchain on remote"

    _ssh "
        if command -v cargo &>/dev/null; then
            echo \"Rust already installed: \$(rustc --version)\"
        else
            echo 'Installing Rust via rustup...'
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
            source \$HOME/.cargo/env
            echo \"Installed: \$(rustc --version)\"
        fi

        # Ensure build deps
        if command -v dnf &>/dev/null; then
            $SUDO dnf install -y gcc openssl-devel pkg-config 2>&1 | tail -1
        elif command -v apt-get &>/dev/null; then
            $SUDO apt-get install -y build-essential libssl-dev pkg-config 2>&1 | tail -1
        fi
    " 2>&1
    info "Rust toolchain ready"
fi

# ── Build on remote ──
if $QUICK_MODE; then
    BUILD_STEP=2
else
    BUILD_STEP=3
fi
step "Step ${BUILD_STEP}/${TOTAL_STEPS}: Building release binary on remote"

_ssh "
    source \$HOME/.cargo/env 2>/dev/null || true
    cd $REMOTE_DIR
    cargo build --release 2>&1 | tail -5
    strip target/release/zorvia 2>/dev/null || true
    ls -lh target/release/zorvia
" 2>&1
info "Binary built on remote"

# ── Install binary + service ──
if $QUICK_MODE; then
    INSTALL_STEP=3
else
    INSTALL_STEP=4
fi
step "Step ${INSTALL_STEP}/${TOTAL_STEPS}: Installing binary and service"

_ssh "
    cd $REMOTE_DIR

    # Stop host systemd front door — K8s NodePort HTTPS is primary (Veyron-style)
    $SUDO systemctl disable --now zorvia-web.service 2>/dev/null || true

    # Install binary (CLI / optional bare-metal)
    $SUDO cp target/release/zorvia /usr/local/bin/zorvia
    $SUDO chmod 755 /usr/local/bin/zorvia
    $SUDO cp deploy/zorvia-web.service /etc/systemd/system/zorvia-web.service
    $SUDO systemctl daemon-reload

    # Scoped kubeconfig for local CLI use
    $SUDO mkdir -p /etc/zorvia
    if [ -f /etc/rancher/k3s/k3s.yaml ]; then
        $SUDO cp /etc/rancher/k3s/k3s.yaml /etc/zorvia/kubeconfig
        $SUDO chown root:${USER} /etc/zorvia/kubeconfig
        $SUDO chmod 640 /etc/zorvia/kubeconfig
        echo 'kubeconfig: installed at /etc/zorvia/kubeconfig'
    fi

    if [ ! -f /etc/zorvia/env ]; then
        API_KEY=\$(openssl rand -hex 32)
        echo \"ZORVIA_API_KEY=\${API_KEY}\" | $SUDO tee /etc/zorvia/env > /dev/null
        $SUDO chmod 600 /etc/zorvia/env
        echo \"API key generated: \${API_KEY}\"
    else
        echo 'API key: already configured (kept existing)'
    fi

    # Build/import local image for k3s (zorvia:local) when container tooling exists.
    # Prefer docker; fall back to podman (common on Ubuntu labs without docker).
    # Use ubuntu:24.04 so host-built binaries (glibc 2.39+) run.
    BUILDER=""
    if command -v docker >/dev/null 2>&1; then
        BUILDER=docker
    elif command -v podman >/dev/null 2>&1; then
        BUILDER=podman
    fi
    if [ -n "$BUILDER" ] && [ -x target/release/zorvia ]; then
        mkdir -p /tmp/zorvia-img/web
        cp -f target/release/zorvia /tmp/zorvia-img/zorvia
        cp -f deploy/Dockerfile.local /tmp/zorvia-img/Dockerfile
        if [ -d web/dist ] && [ -f web/dist/index.html ]; then
            rm -rf /tmp/zorvia-img/web
            cp -a web/dist /tmp/zorvia-img/web
            echo "web UI: bundled from web/dist"
        else
            echo '<!DOCTYPE html><html><body><p>Zorvia UI not built. Run npm run build in web/.</p></body></html>' \
              > /tmp/zorvia-img/web/index.html
            echo 'web UI: placeholder (web/dist missing)'
        fi
        # Podman often tags as localhost/zorvia:local — normalize for imagePullPolicy: Never
        $SUDO "$BUILDER" build -t zorvia:local /tmp/zorvia-img
        $SUDO "$BUILDER" save zorvia:local | $SUDO k3s ctr images import - 2>/dev/null \
          || $SUDO "$BUILDER" save zorvia:local | $SUDO ctr -n k8s.io images import - 2>/dev/null \
          || true
        $SUDO k3s ctr images tag zorvia:local docker.io/library/zorvia:local 2>/dev/null || true
        $SUDO k3s ctr images tag localhost/zorvia:local zorvia:local 2>/dev/null || true
        $SUDO k3s ctr images tag localhost/zorvia:local docker.io/library/zorvia:local 2>/dev/null || true
        echo "image: zorvia:local built/imported via $BUILDER"
        $SUDO kubectl -n zorvia-system rollout restart deployment/zorvia-api 2>/dev/null || true
    else
        echo 'docker/podman/binary missing; using existing zorvia:local if present'
    fi

    # Apply Kubernetes HTTPS manifests (in-pod TLS + NodePort 30152)
    export KUBECONFIG=/etc/rancher/k3s/k3s.yaml
    if command -v kubectl >/dev/null 2>&1 || [ -x /usr/local/bin/kubectl ]; then
        $SUDO kubectl apply -f deploy/k8s.yaml
        # Clean up legacy namespace/manifest names from earlier HTTP NodePort deploys
        $SUDO kubectl delete namespace zorvia --ignore-not-found 2>/dev/null || true
        if [ -d /var/lib/rancher/k3s/server/manifests ]; then
            $SUDO cp deploy/k3s-zorvia-web.yaml /var/lib/rancher/k3s/server/manifests/zorvia-web.yaml \
              || $SUDO cp deploy/k3s-zorvia-web.yaml /var/lib/rancher/k3s/server/manifests/zorvia-api.yaml \
              || true
            echo 'k3s auto-manifest: installed'
        fi
        $SUDO kubectl -n zorvia-system rollout status deployment/zorvia-api --timeout=180s || true
        echo 'k8s: zorvia-api applied (HTTPS NodePort 30152)'
    else
        echo 'kubectl not found; skipped Kubernetes apply'
    fi
" 2>&1
info "Binary and Kubernetes manifests installed"

# ── Verify ──
if $QUICK_MODE; then
    VERIFY_STEP=3
else
    VERIFY_STEP=5
fi
step "Step ${VERIFY_STEP}/${TOTAL_STEPS}: Verifying deployment"

sleep 3
_ssh "
    echo \"Binary:  \$(which zorvia 2>/dev/null || echo NOT_FOUND)\"
    echo \"Version: \$(zorvia --version 2>/dev/null || echo FAILED)\"
    echo \"Host systemd zorvia-web: \$(systemctl is-active zorvia-web 2>/dev/null || echo inactive)\"
    echo \"\"
    $SUDO kubectl -n zorvia-system get deploy,svc,pods 2>/dev/null || true
    echo \"\"
    if curl -sk -o /dev/null -w 'NodePort health: HTTP %{http_code}\n' --max-time 10 https://127.0.0.1:30152/api/v1/health; then
        :
    else
        echo 'NodePort 30152: not ready yet'
        $SUDO kubectl -n zorvia-system describe pods -l app.kubernetes.io/component=api 2>/dev/null | tail -40 || true
    fi
" 2>&1

echo ""
echo "  ============================================"
echo "  Deployment complete: ${USER}@${HOST}"
echo "  ============================================"
echo ""
echo "  Connect:"
echo "    ssh ${USER}@${HOST}"
echo ""
echo "  Web Dashboard (Kubernetes HTTPS, Veyron-style NodePort):"
echo "    https://${HOST}:30152"
echo "    https://${HOST}:30152/api/v1/health"
echo ""
echo "  Service management:"
echo "    sudo kubectl -n zorvia-system get pods,svc"
echo "    sudo kubectl -n zorvia-system logs -l app.kubernetes.io/component=api -f"
echo ""
