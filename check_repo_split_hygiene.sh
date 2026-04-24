#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HARNESS_CARGO="${ROOT_DIR}/Cargo.toml"
HARNESS_LOCK="${ROOT_DIR}/Cargo.lock"
HARNESS_README="${ROOT_DIR}/README.md"
HARNESS_TEST="${ROOT_DIR}/tests/cross_repo_smoke.rs"
WORKSPACE_SMOKE="${ROOT_DIR}/run_workspace_smoke.sh"
PIN_WORKSPACE_REVS="${ROOT_DIR}/pin_workspace_revs.sh"

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
check_absent 'substrate::policy::advanced|policy::advanced::|PolicyAttrs|PolicySignals|ContextValue|ContextId|set_policy_attrs' \
  "cross-repo locked lane must not depend on post-rev advanced policy metadata paths" \
  "${HARNESS_TEST}"
check_absent '[0-9a-f]{40}' \
  "cross-repo tests must derive rev expectations from Cargo.toml instead of hard-coding release SHAs" \
  "${HARNESS_TEST}"

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

if [[ ! -x "${PIN_WORKSPACE_REVS}" ]]; then
  echo "cross-repo harness must provide an executable release rev pinning runner" >&2
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

if ! grep -Fq './pin_workspace_revs.sh' "${HARNESS_README}"; then
  echo "cross-repo harness README must document the release rev pinning lane" >&2
  FAILED=1
fi

if ! grep -Fq 'HIBANA_CROSS_REPO_WORKSPACE_PATCHED=run_workspace_smoke.sh' "${WORKSPACE_SMOKE}"; then
  echo "cross-repo workspace smoke runner must stamp patched source-read mode" >&2
  FAILED=1
fi

check_absent 'LOCKFILE_BACKUP|mv "\$\{LOCKFILE_BACKUP\}" "\$\{ROOT_DIR\}/Cargo\.lock"' \
  "cross-repo workspace smoke must not mutate the checkout lockfile" \
  "${WORKSPACE_SMOKE}"

if ! grep -Fq 'SMOKE_DIR="$(mktemp -d)"' "${WORKSPACE_SMOKE}"; then
  echo "cross-repo workspace smoke runner must execute from an isolated temp harness" >&2
  FAILED=1
fi

if ! grep -Fq 'cargo +"${TOOLCHAIN}" check --no-default-features' "${WORKSPACE_SMOKE}"; then
  echo "cross-repo workspace smoke runner must run sibling no-default-features checks" >&2
  FAILED=1
fi

if ! grep -Fq 'cargo +"${TOOLCHAIN}" test --features std' "${WORKSPACE_SMOKE}"; then
  echo "cross-repo workspace smoke runner must run sibling std tests" >&2
  FAILED=1
fi

if ! grep -Fq 'release pinning refuses dirty sibling checkout' "${PIN_WORKSPACE_REVS}"; then
  echo "cross-repo release rev pinning runner must reject dirty sibling checkouts" >&2
  FAILED=1
fi

if ! grep -Fq 'release pinning requires published GitHub ref' "${PIN_WORKSPACE_REVS}"; then
  echo "cross-repo release rev pinning runner must reject unpublished sibling commits" >&2
  FAILED=1
fi

if ! grep -Fq 'cargo +"${TOOLCHAIN}" test --locked' "${PIN_WORKSPACE_REVS}"; then
  echo "cross-repo release rev pinning runner must validate the locked GitHub lane" >&2
  FAILED=1
fi

if ! grep -Fq 'WORKSPACE_PATCH_SENTINEL' "${HARNESS_TEST}"; then
  echo "cross-repo tests must gate local source reads behind the workspace patch sentinel" >&2
  FAILED=1
fi

if ! grep -Fq 'assert_no_old_surface_paths' "${HARNESS_TEST}"; then
  echo "cross-repo tests must forbid old g/substrate surface paths in patched siblings" >&2
  FAILED=1
fi

if [[ "${FAILED}" -ne 0 ]]; then
  exit 1
fi

echo "cross-repo split hygiene check passed"
