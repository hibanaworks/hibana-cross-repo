#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HARNESS_CARGO="${ROOT_DIR}/Cargo.toml"
HARNESS_LOCK="${ROOT_DIR}/Cargo.lock"
HARNESS_README="${ROOT_DIR}/README.md"
HARNESS_TEST="${ROOT_DIR}/tests/cross_repo_smoke.rs"
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

check_absent '\[patch\.crates-io\]' \
  "cross-repo harness must not rely on a local crates.io patch overlay" \
  "${HARNESS_CARGO}" \
  "${HARNESS_README}"
check_absent 'branch *= *"main"' \
  "cross-repo harness must not float on moving main branches" \
  "${HARNESS_CARGO}"
check_absent '^(hibana|hibana-epf|hibana-mgmt)[[:space:]]*=.*path *= *"\.\./' \
  "cross-repo harness default lane must not use direct path dependencies" \
  "${HARNESS_CARGO}" \
  "${HARNESS_README}"
check_absent '\.\./\.\./hibana/tests/|../hibana-epf|../hibana-mgmt' \
  "cross-repo harness docs must not assume sibling checkout layout" \
  "${HARNESS_README}"

for required in \
  'hibana = { git = "https://github.com/hibanaworks/hibana"' \
  'hibana-epf = { git = "https://github.com/hibanaworks/hibana-epf"' \
  'hibana-mgmt = { git = "https://github.com/hibanaworks/hibana-mgmt"'
do
  if ! grep -Fq "${required}" "${HARNESS_CARGO}"; then
    echo "cross-repo harness must depend on the GitHub sibling repo: ${required}" >&2
    FAILED=1
  fi
done

for required in \
  'hibana = { git = "https://github.com/hibanaworks/hibana", rev = "' \
  'hibana-epf = { git = "https://github.com/hibanaworks/hibana-epf", rev = "' \
  'hibana-mgmt = { git = "https://github.com/hibanaworks/hibana-mgmt", rev = "'
do
  if ! grep -Fq "${required}" "${HARNESS_CARGO}"; then
    echo "cross-repo harness must pin each sibling repo to an immutable manifest rev: ${required}" >&2
    FAILED=1
  fi
done

for required in \
  'source = "git+https://github.com/hibanaworks/hibana?rev=' \
  'source = "git+https://github.com/hibanaworks/hibana-epf?rev=' \
  'source = "git+https://github.com/hibanaworks/hibana-mgmt?rev='
do
  if ! grep -Fq "${required}" "${HARNESS_LOCK}"; then
    echo "cross-repo harness lockfile must pin the resolved git source: ${required}" >&2
    FAILED=1
  fi
done

if [[ ! -x "${WORKSPACE_SMOKE}" ]]; then
  echo "cross-repo harness must provide a workspace smoke runner" >&2
  FAILED=1
fi

if ! grep -Fq 'cargo test --locked' "${HARNESS_README}"; then
  echo "cross-repo harness README must document the locked exact-GitHub lane" >&2
  FAILED=1
fi

if ! grep -Fq './run_workspace_smoke.sh' "${HARNESS_README}"; then
  echo "cross-repo harness README must document the explicit workspace smoke lane" >&2
  FAILED=1
fi

if ! grep -Fq 'HIBANA_CROSS_REPO_WORKSPACE_PATCHED=run_workspace_smoke.sh' "${WORKSPACE_SMOKE}"; then
  echo "cross-repo workspace smoke runner must stamp patched source-read mode" >&2
  FAILED=1
fi

if ! grep -Fq 'WORKSPACE_PATCH_SENTINEL' "${HARNESS_TEST}"; then
  echo "cross-repo tests must gate local source reads behind the workspace patch sentinel" >&2
  FAILED=1
fi

if [[ "${FAILED}" -ne 0 ]]; then
  exit 1
fi

echo "cross-repo split hygiene check passed"
