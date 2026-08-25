#!/usr/bin/env bash
# api-test.sh — libvirt VM API smoke test (backward-compatible wrapper)
#
# Usage:
#   ./scripts/api-test.sh [BASE_URL] [USER] [PASS]
#
# Examples:
#   VSPASS=max ./scripts/api-test.sh https://185.165.240.5:5092 sus
#
# Runs e2e-test.sh with libvirt-only mode (no OpenStack / preflight).
#
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "${SCRIPT_DIR}/e2e-test.sh" "$@" --libvirt-only
