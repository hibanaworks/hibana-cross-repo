#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HARNESS_CARGO="${ROOT_DIR}/Cargo.toml"
HARNESS_README="${ROOT_DIR}/README.md"
WORKSPACE_SMOKE="${ROOT_DIR}/run_workspace_smoke.sh"

FAILED=0

check_absent() {
  local pattern="$1"
  local label="$2"
  shift 2
  if rg -n "${pattern}" "$@"; then
    echo "cross-repo boundary violation: ${label}" >&2
    FAILED=1
  fi
}

check_absent 'path *= *"(\.\./|/Users/)' \
  "cross-repo harness must not depend on local path manifests" \
  "${HARNESS_CARGO}"
check_absent '\[patch\.crates-io\]' \
  "cross-repo harness must not rely on a local crates.io patch overlay" \
  "${HARNESS_CARGO}" \
  "${HARNESS_README}"
check_absent 'branch *= *"main"' \
  "cross-repo harness must not float on moving main branches" \
  "${HARNESS_CARGO}"
check_absent '\.\./\.\./hibana/tests/|../hibana-epf|../hibana-mgmt' \
  "cross-repo harness docs must not assume sibling checkout layout" \
  "${HARNESS_README}" \
  "${ROOT_DIR}/tests"

for required in \
  'git = "https://github.com/hibanaworks/hibana"' \
  'git = "https://github.com/hibanaworks/hibana-epf"' \
  'git = "https://github.com/hibanaworks/hibana-mgmt"'
do
  if ! grep -Fq "${required}" "${HARNESS_CARGO}"; then
    echo "cross-repo harness must pin GitHub repo dependency: ${required}" >&2
    FAILED=1
  fi
done

extract_rev() {
  local dep="$1"
  local pinned
  pinned="$(sed -nE "s/^${dep}[[:space:]]*=.*rev = \"([0-9a-f]+)\".*/\\1/p" "${HARNESS_CARGO}")"
  if [[ -z "${pinned}" ]]; then
    echo "cross-repo harness must pin immutable dependency revision for ${dep}" >&2
    FAILED=1
    return 1
  fi
  printf '%s\n' "${pinned}"
}

HIBANA_REV="$(extract_rev hibana || true)"
HIBANA_EPF_REV="$(extract_rev hibana-epf || true)"
HIBANA_MGMT_REV="$(extract_rev hibana-mgmt || true)"

for pinned in "${HIBANA_REV}" "${HIBANA_EPF_REV}" "${HIBANA_MGMT_REV}"; do
  if [[ -n "${pinned}" ]] && [[ ! "${pinned}" =~ ^[0-9a-f]{40}$ ]]; then
    echo "cross-repo harness must use full immutable git revisions: ${pinned}" >&2
    FAILED=1
  fi
done

if [[ ! -x "${WORKSPACE_SMOKE}" ]]; then
  echo "cross-repo harness must provide a workspace smoke runner" >&2
  FAILED=1
fi

if ! grep -Fq './run_workspace_smoke.sh' "${HARNESS_README}"; then
  echo "cross-repo harness README must document the workspace smoke lane" >&2
  FAILED=1
fi

if [[ "${FAILED}" -ne 0 ]]; then
  exit 1
fi

echo "cross-repo split hygiene check passed"
