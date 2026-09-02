#!/usr/bin/env bash
set -Eeuo pipefail

readonly ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
readonly SEED_FILE="${ROOT_DIR}/scripts/seed_demo.sql"
readonly DEFAULT_DATABASE_URL="postgres://app:secret@127.0.0.1:5432/xiaoluoquiz"

if ! command -v psql >/dev/null 2>&1; then
    printf 'error: required command not found: psql\n' >&2
    exit 127
fi

if [[ ! -f "${SEED_FILE}" ]]; then
    printf 'error: seed file not found: %s\n' "${SEED_FILE}" >&2
    exit 1
fi

export DATABASE_URL="${DATABASE_URL:-${DEFAULT_DATABASE_URL}}"
psql "${DATABASE_URL}" --set ON_ERROR_STOP=1 --file "${SEED_FILE}"
printf 'demo questions seeded into the configured database\n'
