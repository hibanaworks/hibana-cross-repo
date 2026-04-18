#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${ROOT_DIR}"

HIBANA_DIR="${ROOT_DIR}/../hibana"
HIBANA_EPF_DIR="${ROOT_DIR}/../hibana-epf"
HIBANA_MGMT_DIR="${ROOT_DIR}/../hibana-mgmt"

for required in "${HIBANA_DIR}" "${HIBANA_EPF_DIR}" "${HIBANA_MGMT_DIR}"; do
  if [[ ! -d "${required}" ]]; then
    echo "workspace smoke requires sibling checkout: ${required}" >&2
    exit 1
  fi
done

cargo test \
  --config "patch.\"https://github.com/hibanaworks/hibana\".hibana.path=\"${HIBANA_DIR}\"" \
  --config "patch.\"https://github.com/hibanaworks/hibana-epf\".hibana-epf.path=\"${HIBANA_EPF_DIR}\"" \
  --config "patch.\"https://github.com/hibanaworks/hibana-mgmt\".hibana-mgmt.path=\"${HIBANA_MGMT_DIR}\""
