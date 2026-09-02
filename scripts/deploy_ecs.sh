#!/usr/bin/env bash
set -Eeuo pipefail

readonly ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
readonly ENV_FILE="${ENV_FILE:-${ROOT_DIR}/.env}"
readonly BASE_COMPOSE_FILE="${ROOT_DIR}/compose.production.yaml"
readonly POSTGRES_COMPOSE_FILE="${ROOT_DIR}/compose.production.postgres.yaml"

fail() {
    printf 'error: %s\n' "$1" >&2
    exit 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || fail "required command not found: $1"
}

dotenv_value() {
    local name="$1"
    awk -v name="$name" '
        $0 ~ "^[[:space:]]*" name "[[:space:]]*=" {
            sub("^[[:space:]]*" name "[[:space:]]*=[[:space:]]*", "")
            sub("[[:space:]]+#.*$", "")
            gsub(/^"/, "", $0)
            gsub(/"$/, "", $0)
            gsub(/^\047/, "", $0)
            gsub(/\047$/, "", $0)
            print
            exit
        }
    ' "${ENV_FILE}"
}

config_value() {
    local name="$1"
    if [[ -v "${name}" ]]; then
        printf '%s' "${!name}"
    else
        dotenv_value "${name}"
    fi
}

require_value() {
    local name="$1"
    local value
    value="$(config_value "${name}")"
    [[ -n "${value}" ]] || fail "${name} must be set in ${ENV_FILE} or the environment"
    [[ "${value}" != *replace-with-* ]] || fail "replace the example value for ${name} before deploying"
    printf '%s' "${value}"
}

require_command docker
[[ -f "${ENV_FILE}" ]] || fail "environment file not found: ${ENV_FILE}; copy .env.example to .env first"
[[ -f "${BASE_COMPOSE_FILE}" ]] || fail "Compose file not found: ${BASE_COMPOSE_FILE}"

readonly use_local_postgres="$(config_value USE_LOCAL_POSTGRES)"
readonly database_url="$(require_value DATABASE_URL)"
readonly initial_password="$(require_value INITIAL_PASSWORD)"
readonly image="$(require_value XIAOLUOQUIZ_IMAGE)"
readonly domain="$(config_value DOMAIN)"

[[ -n "${initial_password}" ]] || fail "INITIAL_PASSWORD must not be empty"
[[ -n "${image}" ]] || fail "XIAOLUOQUIZ_IMAGE must not be empty"
[[ -n "${domain}" ]] || fail "DOMAIN must be set in ${ENV_FILE} or the environment"

compose_args=(--env-file "${ENV_FILE}" -f "${BASE_COMPOSE_FILE}")
pull_services=(app caddy)

case "${use_local_postgres}" in
    1|true|TRUE|yes|YES)
        [[ -f "${POSTGRES_COMPOSE_FILE}" ]] || fail "local PostgreSQL Compose file not found: ${POSTGRES_COMPOSE_FILE}"
        compose_args+=( -f "${POSTGRES_COMPOSE_FILE}" )
        pull_services+=(postgres)
        postgres_password="$(require_value POSTGRES_PASSWORD)"
        [[ -n "${postgres_password}" ]] || fail "POSTGRES_PASSWORD must not be empty"
        [[ "${database_url}" == *"@postgres:"* ]] || fail "DATABASE_URL must use the postgres service when USE_LOCAL_POSTGRES=1"
        ;;
    0|false|FALSE|no|NO|'')
        ;;
    *)
        fail "USE_LOCAL_POSTGRES must be 0 or 1"
        ;;
esac

compose() {
    docker compose "${compose_args[@]}" "$@"
}

compose config --quiet || fail "Compose configuration validation failed"

if [[ "${BUILD_IMAGE:-0}" == "1" ]]; then
    printf 'Building application image: %s\n' "${image}"
    compose build app
else
    printf 'Pulling deployment images...\n'
    compose pull "${pull_services[@]}"
fi

printf 'Running database migrations...\n'
compose run --rm migrate

printf 'Starting application and reverse proxy...\n'
compose up --detach --remove-orphans app caddy

compose ps
