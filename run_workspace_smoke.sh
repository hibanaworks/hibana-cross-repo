#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TOOLCHAIN="${TOOLCHAIN:-1.95.0}"
SMOKE_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "${SMOKE_DIR}"
}

trap cleanup EXIT

HIBANA_DIR="${ROOT_DIR}/../hibana"
HIBANA_EPF_DIR="${ROOT_DIR}/../hibana-epf"
HIBANA_MGMT_DIR="${ROOT_DIR}/../hibana-mgmt"

for required in "${HIBANA_DIR}" "${HIBANA_EPF_DIR}" "${HIBANA_MGMT_DIR}"; do
  if [[ ! -d "${required}" ]]; then
    echo "workspace smoke requires sibling checkout: ${required}" >&2
    exit 1
  fi
done

run_sibling_crate() {
  local crate_dir="$1"
  (
    cd "${crate_dir}"
    cargo +"${TOOLCHAIN}" check --no-default-features
    cargo +"${TOOLCHAIN}" test --features std
  )
}

mkdir -p "${SMOKE_DIR}/src" "${SMOKE_DIR}/tests"
cp "${ROOT_DIR}/Cargo.toml" "${SMOKE_DIR}/Cargo.toml"
cp "${ROOT_DIR}/Cargo.lock" "${SMOKE_DIR}/Cargo.lock"
cp "${ROOT_DIR}/README.md" "${SMOKE_DIR}/README.md"
cp "${ROOT_DIR}/src/lib.rs" "${SMOKE_DIR}/src/lib.rs"
cp "${ROOT_DIR}/tests/cross_repo_smoke.rs" "${SMOKE_DIR}/tests/cross_repo_smoke.rs"

cd "${SMOKE_DIR}"

HIBANA_CROSS_REPO_WORKSPACE_SMOKE=1 \
HIBANA_CROSS_REPO_WORKSPACE_PATCHED=run_workspace_smoke.sh \
HIBANA_CROSS_REPO_HIBANA_DIR="${HIBANA_DIR}" \
HIBANA_CROSS_REPO_HIBANA_EPF_DIR="${HIBANA_EPF_DIR}" \
HIBANA_CROSS_REPO_HIBANA_MGMT_DIR="${HIBANA_MGMT_DIR}" \
cargo +"${TOOLCHAIN}" test \
  --config "patch.\"https://github.com/hibanaworks/hibana\".hibana.path=\"${HIBANA_DIR}\"" \
  --config "patch.\"https://github.com/hibanaworks/hibana-epf\".hibana-epf.path=\"${HIBANA_EPF_DIR}\"" \
  --config "patch.\"https://github.com/hibanaworks/hibana-mgmt\".hibana-mgmt.path=\"${HIBANA_MGMT_DIR}\""

run_sibling_crate "${HIBANA_EPF_DIR}"
run_sibling_crate "${HIBANA_MGMT_DIR}"
