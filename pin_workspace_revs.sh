#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TOOLCHAIN="${TOOLCHAIN:-1.95.0}"
MANIFEST="${ROOT_DIR}/Cargo.toml"

repo_dir() {
  local name="$1"
  printf '%s\n' "${ROOT_DIR}/../${name}"
}

repo_rev() {
  local dir="$1"
  git -C "${dir}" rev-parse HEAD
}

repo_url() {
  local name="$1"
  printf 'https://github.com/hibanaworks/%s.git\n' "${name}"
}

require_clean_repo() {
  local name="$1"
  local dir="$2"
  if [[ ! -d "${dir}/.git" ]]; then
    echo "release pinning requires sibling git checkout: ${dir}" >&2
    exit 1
  fi
  if [[ -n "$(git -C "${dir}" status --porcelain=v1)" ]]; then
    echo "release pinning refuses dirty sibling checkout: ${name}" >&2
    exit 1
  fi
}

require_published_rev() {
  local name="$1"
  local rev="$2"
  local url
  url="$(repo_url "${name}")"
  if ! git ls-remote "${url}" | awk '{print $1}' | grep -Fxq "${rev}"; then
    echo "release pinning requires published GitHub ref for ${name}: ${rev}" >&2
    exit 1
  fi
}

replace_rev() {
  local crate="$1"
  local rev="$2"
  perl -0pi -e \
    "s#${crate} = \\{ git = \"https://github.com/hibanaworks/${crate}\", rev = \"[0-9a-f]{40}\"#${crate} = { git = \"https://github.com/hibanaworks/${crate}\", rev = \"${rev}\"#" \
    "${MANIFEST}"
}

cd "${ROOT_DIR}"

HIBANA_DIR="$(repo_dir hibana)"
HIBANA_EPF_DIR="$(repo_dir hibana-epf)"
HIBANA_MGMT_DIR="$(repo_dir hibana-mgmt)"

require_clean_repo hibana "${HIBANA_DIR}"
require_clean_repo hibana-epf "${HIBANA_EPF_DIR}"
require_clean_repo hibana-mgmt "${HIBANA_MGMT_DIR}"

HIBANA_REV="$(repo_rev "${HIBANA_DIR}")"
HIBANA_EPF_REV="$(repo_rev "${HIBANA_EPF_DIR}")"
HIBANA_MGMT_REV="$(repo_rev "${HIBANA_MGMT_DIR}")"

require_published_rev hibana "${HIBANA_REV}"
require_published_rev hibana-epf "${HIBANA_EPF_REV}"
require_published_rev hibana-mgmt "${HIBANA_MGMT_REV}"

replace_rev hibana "${HIBANA_REV}"
replace_rev hibana-epf "${HIBANA_EPF_REV}"
replace_rev hibana-mgmt "${HIBANA_MGMT_REV}"

cargo +"${TOOLCHAIN}" update -p hibana
cargo +"${TOOLCHAIN}" update -p hibana-epf
cargo +"${TOOLCHAIN}" update -p hibana-mgmt

cargo +"${TOOLCHAIN}" test --locked
bash "${ROOT_DIR}/check_repo_split_hygiene.sh"
