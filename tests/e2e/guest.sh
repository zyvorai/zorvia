#!/usr/bin/env bash
# Real-guest E2E against a running Zorvia API on a KVM-capable cluster.
#
# Unlike the kind smoke job this boots a guest and drives it through the API:
# create+boot (guest agent up) -> snapshot/revert -> live migration under CPU
# load -> API restart with the guest untouched. Each scenario prints one
# markdown row (PASS / FAIL / SKIP + seconds) so a run can be pasted into
# docs/SUPPORT_MATRIX.md. Exit status is non-zero if any scenario FAILed.
#
#   ZORVIA_LAB_HOST=80.79.5.173 ZORVIA_E2E_PASSWORD=... tests/e2e/guest.sh
#
# Env: ZORVIA_LAB_HOST/PORT, ZORVIA_E2E_USER (admin) + ZORVIA_E2E_PASSWORD or
#      ZORVIA_E2E_TOKEN; E2E_IMAGE (containerdisk), E2E_VM (name prefix);
#      KUBECTL (e.g. "ssh root@host KUBECONFIG=/etc/rancher/k3s/k3s.yaml kubectl")
#      enables the node check (E2E_NAMESPACE = the API's default namespace) and the API-restart scenario.
set -uo pipefail

HOST="${ZORVIA_LAB_HOST:?set ZORVIA_LAB_HOST}"
BASE="https://${HOST}:${ZORVIA_LAB_PORT:-30152}"
IMAGE="${E2E_IMAGE:-quay.io/containerdisks/ubuntu:24.04}"
VM="${E2E_VM:-zorvia-e2e}-$RANDOM"
KUBECTL="${KUBECTL:-}"
NS="${E2E_NAMESPACE:-default}"   # the namespace the API deploys VMs into
FAILS=0

TOKEN="${ZORVIA_E2E_TOKEN:-}"
if [ -z "$TOKEN" ]; then
  : "${ZORVIA_E2E_PASSWORD:?set ZORVIA_E2E_PASSWORD or ZORVIA_E2E_TOKEN}"
  TOKEN=$(curl -skf -X POST "$BASE/api/v1/auth/login" -H 'Content-Type: application/json' \
    -d "{\"username\":\"${ZORVIA_E2E_USER:-admin}\",\"password\":\"${ZORVIA_E2E_PASSWORD}\"}" | jq -r .token)
fi
[ -n "$TOKEN" ] && [ "$TOKEN" != null ] || { echo "login failed" >&2; exit 2; }

api() { # method path [json]
  curl -sk -m 900 -X "$1" "$BASE/api$2" -H "Authorization: Bearer $TOKEN" \
    -H 'Content-Type: application/json' ${3:+-d "$3"} -w '\n%{http_code}'
}
code() { tail -n1; }
body() { sed '$d'; }

row() { printf '| %s | %s | %ss | %s |\n' "$1" "$2" "$3" "${4:-}"; [ "$2" = FAIL ] && FAILS=$((FAILS+1)); }
run() { # name fn
  local t0=$SECONDS out rc
  out=$("$2" 2>&1); rc=$?
  case $rc in 0) row "$1" PASS $((SECONDS-t0)) "$out" ;; 77) row "$1" SKIP $((SECONDS-t0)) "$out" ;; *) row "$1" FAIL $((SECONDS-t0)) "$out" ;; esac
}
cleanup() { api DELETE "/vms/$VM" >/dev/null 2>&1; }
trap cleanup EXIT

wait_for() { # seconds cmd...
  local deadline=$((SECONDS+$1)); shift
  until "$@"; do [ $SECONDS -ge $deadline ] && return 1; sleep 5; done
}

vm_running() { api GET "/vms/$VM" | body | jq -e '(.. | strings? | select(. == "Running")) // empty' >/dev/null 2>&1; }

s_create_boot() {
  local r; r=$(api POST /vms "{\"name\":\"$VM\",\"image\":\"$IMAGE\",\"cpus\":2,\"memory\":2048,\"disk\":10,
    \"cloud_init\":{\"user_data\":\"#cloud-config\nruncmd:\n  - [sh, -c, 'while :; do :; done &']\n  - [sh, -c, 'dd if=/dev/zero of=/var/tmp/load bs=1M count=512 conv=fsync; sync']\"}}")
  # The API answers within 30 s; 408 only means the guest was not ready yet.
  case "$(echo "$r" | code)" in 2??|408) ;; *) echo "create: $(echo "$r" | body | head -c 200)"; return 1 ;; esac
  wait_for 900 vm_running || { echo "never Running"; return 1; }
  agent_up() { [ "$(api POST "/vms/$VM/wait-ready" | code)" = 200 ]; }
  wait_for 900 agent_up || { echo "guest agent never came up"; return 1; }
  echo "agent up"
}

s_snapshot_revert() {
  local r id warn
  r=$(api POST "/vms/$VM/snapshots" '{"name":"e2e-snap","description":"e2e"}')
  [ "$(echo "$r" | code)" -lt 300 ] || { echo "snapshot: $(echo "$r" | body | head -c 200)"; return 1; }
  id=$(echo "$r" | body | jq -r .id)   # the API prefixes the VM name
  listed() { api GET "/vms/$VM/snapshots" | body | jq -e --arg id "$id" 'any(.[]; .id == $id)' >/dev/null 2>&1; }
  wait_for 120 listed || { echo "snapshot $id never listed"; return 1; }
  # A containerdisk guest has no PVC, so the snapshot holds configuration only;
  # the API says so in `warning`. Report it rather than calling that a data snapshot.
  warn=$(api GET "/vms/$VM/snapshots" | body | jq -r --arg id "$id" '.[] | select(.id == $id) | .warning // empty')
  r=$(api DELETE "/vms/$VM/snapshots/$id"); [ "$(echo "$r" | code)" -lt 300 ] || { echo "delete: $(echo "$r" | body | head -c 200)"; return 1; }
  [ -z "$warn" ] && echo "created, listed, deleted" || echo "created, listed, deleted (config-only: no PVC-backed disk)"
}

s_live_migration() {
  local nodes
  if [ -n "$KUBECTL" ]; then
    nodes=$($KUBECTL get nodes --no-headers 2>/dev/null | grep -vc SchedulingDisabled)
    [ "${nodes:-1}" -ge 2 ] || { echo "single schedulable node"; return 77; }
  else
    echo "KUBECTL not set; cannot confirm a second node"; return 77
  fi
  local before after r
  before=$($KUBECTL -n "$NS" get vmi "$VM" -o jsonpath='{.status.nodeName}' 2>/dev/null)
  r=$(api POST "/vms/$VM/migrate"); [ "$(echo "$r" | code)" -lt 300 ] || { echo "migrate: $(echo "$r" | body | head -c 200)"; return 1; }
  moved() { after=$($KUBECTL -n "$NS" get vmi "$VM" -o jsonpath='{.status.nodeName}' 2>/dev/null); [ -n "$after" ] && [ "$after" != "$before" ]; }
  wait_for 600 moved || { echo "still on $before"; return 1; }
  # guest must still answer after the move (the load loop keeps it busy)
  agent_up || wait_for 300 agent_up || { echo "agent lost after migration"; return 1; }
  echo "$before -> $after under load"
}

s_api_restart() {
  [ -n "$KUBECTL" ] || { echo "KUBECTL not set"; return 77; }
  $KUBECTL -n zorvia-system rollout restart deploy/zorvia-api >/dev/null 2>&1 || { echo "restart failed"; return 1; }
  up() { curl -skf -m 5 "$BASE/api/v1/health" >/dev/null 2>&1; }
  sleep 5
  wait_for 600 up || { echo "API did not return"; return 1; }
  # old token must still be valid (token_version lives in the DB) and the guest untouched
  [ "$(api GET "/vms/$VM" | code)" = 200 ] || { echo "session or VM lost after restart"; return 1; }
  vm_running || { echo "VM not Running after API restart"; return 1; }
  echo "session kept, VM untouched"
}

echo "| scenario | result | time | detail |"; echo "|---|---|---|---|"
run "create + boot + guest agent ($IMAGE)" s_create_boot
run "snapshot create/delete" s_snapshot_revert
run "live migration under load" s_live_migration
run "API restart, guest untouched" s_api_restart
echo; echo "VM: $VM  host: $HOST  $(date -u +%FT%TZ)"
exit $((FAILS > 0))
