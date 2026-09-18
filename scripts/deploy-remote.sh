#!/usr/bin/env bash
# Thin wrapper — canonical script lives at deploy/remote-deploy.sh
exec "$(cd "$(dirname "$0")/.." && pwd)/deploy/remote-deploy.sh" "$@"
