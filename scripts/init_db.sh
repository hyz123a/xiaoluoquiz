#!/usr/bin/env bash
set -Eeuo pipefail

readonly ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
readonly MIGRATIONS_DIR="${ROOT_DIR}/migrations"
# Override this with a pinned tag in CI or deployment environments.
readonly POSTGRES_IMAGE="${POSTGRES_IMAGE:-postgres:latest}"
readonly DB_PORT="${DB_PORT:-5432}"
readonly SUPERUSER="${SUPERUSER:-postgres}"
readonly SUPERUSER_PWD="${SUPERUSER_PWD:-password}"
readonly APP_USER="${APP_USER:-app}"
readonly APP_USER_PWD="${APP_USER_PWD:-secret}"
readonly APP_DB_NAME="${APP_DB_NAME:-xiaoluoquiz}"
readonly CONTAINER_NAME="${CONTAINER_NAME:-xiaoluoquiz-postgres}"

require_command() {
    local command_name="$1"
    if ! command -v "${command_name}" >/dev/null 2>&1; then
        printf 'error: required command not found: %s\n' "${command_name}" >&2
        exit 127
    fi
}

wait_for_postgres() {
    local health
    for _ in $(seq 1 60); do
        health="$(docker inspect -f '{{.State.Health.Status}}' "${CONTAINER_NAME}")"
        if [[ "${health}" == healthy ]]; then
            return 0
        fi
        sleep 1
    done

    printf 'error: PostgreSQL container did not become healthy\n' >&2
    docker logs "${CONTAINER_NAME}" >&2
    exit 1
}

require_command sqlx

if [[ -z "${SKIP_DOCKER:-}" ]]; then
    require_command docker

    if docker ps -a --format '{{.Names}}' | grep -Fxq "${CONTAINER_NAME}"; then
        printf 'error: container already exists: %s\n' "${CONTAINER_NAME}" >&2
        printf 'hint: remove it or rerun with SKIP_DOCKER=1 and a matching DATABASE_URL\n' >&2
        exit 1
    fi

    docker run --detach --name "${CONTAINER_NAME}" \
        --env "POSTGRES_USER=${SUPERUSER}" \
        --env "POSTGRES_PASSWORD=${SUPERUSER_PWD}" \
        --health-cmd="pg_isready -U ${SUPERUSER} -d postgres" \
        --health-interval=1s \
        --health-timeout=5s \
        --health-retries=60 \
        --publish "${DB_PORT}:5432" \
        "${POSTGRES_IMAGE}" >/dev/null

    wait_for_postgres

    # The local application role needs CREATEDB so sqlx can create the local database.
    # Production databases should be provisioned by the cloud provider instead.
    docker exec -i "${CONTAINER_NAME}" psql \
        --username "${SUPERUSER}" \
        --dbname postgres \
        --set=app_user="${APP_USER}" \
        --set=app_password="${APP_USER_PWD}" <<'SQL'
SELECT format('CREATE USER %I WITH PASSWORD %L', :'app_user', :'app_password')
WHERE NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = :'app_user')
\gexec

ALTER USER :"app_user" WITH PASSWORD :'app_password' CREATEDB;
SQL

    export DATABASE_URL="postgres://${APP_USER}:${APP_USER_PWD}@127.0.0.1:${DB_PORT}/${APP_DB_NAME}"
else
    if [[ -z "${DATABASE_URL:-}" ]]; then
        printf 'error: DATABASE_URL is required when SKIP_DOCKER is set\n' >&2
        exit 1
    fi
fi

if [[ ! -d "${MIGRATIONS_DIR}" ]]; then
    printf 'error: migration directory not found: %s\n' "${MIGRATIONS_DIR}" >&2
    exit 1
fi

printf 'PostgreSQL is ready; creating database if needed and running migrations...\n'
if [[ -z "${SKIP_DOCKER:-}" ]]; then
    sqlx database create
fi
sqlx migrate run --source "${MIGRATIONS_DIR}"
if [[ -z "${SKIP_DOCKER:-}" ]]; then
    printf 'PostgreSQL has been migrated: %s\n' "${APP_DB_NAME}"
else
    printf 'PostgreSQL has been migrated from DATABASE_URL\n'
fi
