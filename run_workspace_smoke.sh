#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${ROOT_DIR}"
TOOLCHAIN="${TOOLCHAIN:-1.95.0}"
LOCKFILE_BACKUP="$(mktemp)"

cleanup() {
  if [[ -f "${LOCKFILE_BACKUP}" ]]; then
    mv "${LOCKFILE_BACKUP}" "${ROOT_DIR}/Cargo.lock"
  fi
}

cp "${ROOT_DIR}/Cargo.lock" "${LOCKFILE_BACKUP}"
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

HIBANA_CROSS_REPO_WORKSPACE_SMOKE=1 cargo +"${TOOLCHAIN}" test \
  --config "patch.\"https://github.com/hibanaworks/hibana\".hibana.path=\"${HIBANA_DIR}\"" \
  --config "patch.\"https://github.com/hibanaworks/hibana-epf\".hibana-epf.path=\"${HIBANA_EPF_DIR}\"" \
  --config "patch.\"https://github.com/hibanaworks/hibana-mgmt\".hibana-mgmt.path=\"${HIBANA_MGMT_DIR}\""
