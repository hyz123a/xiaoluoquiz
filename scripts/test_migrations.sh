#!/usr/bin/env bash
set -Eeuo pipefail

readonly ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
readonly MIGRATIONS_DIR="${ROOT_DIR}/migrations"
readonly POSTGRES_IMAGE="${POSTGRES_IMAGE:-postgres:latest}"
readonly DB_PORT="${DB_TEST_PORT:-$((55432 + ($$ % 1000)))}"
readonly CONTAINER_NAME="xiaoluoquiz-migrations-test-$$"
readonly DB_USER="${DB_TEST_USER:-app}"
readonly DB_PASSWORD="${DB_TEST_PASSWORD:-secret}"
readonly DB_NAME="${DB_TEST_NAME:-xiaoluoquiz_test}"

cleanup() {
    docker rm -f "${CONTAINER_NAME}" >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM

for command_name in docker sqlx; do
    if ! command -v "${command_name}" >/dev/null 2>&1; then
        printf 'error: required command not found: %s\n' "${command_name}" >&2
        exit 127
    fi
done

if [[ ! -d "${MIGRATIONS_DIR}" ]]; then
    printf 'error: migration directory not found: %s\n' "${MIGRATIONS_DIR}" >&2
    exit 1
fi

docker run --detach --name "${CONTAINER_NAME}" \
    --env "POSTGRES_USER=${DB_USER}" \
    --env "POSTGRES_PASSWORD=${DB_PASSWORD}" \
    --env "POSTGRES_DB=${DB_NAME}" \
    --health-cmd="pg_isready -U ${DB_USER} -d ${DB_NAME}" \
    --health-interval=1s \
    --health-timeout=5s \
    --health-retries=30 \
    --publish "${DB_PORT}:5432" \
    "${POSTGRES_IMAGE}" >/dev/null

for _ in $(seq 1 30); do
    health="$(docker inspect -f '{{.State.Health.Status}}' "${CONTAINER_NAME}")"
    if [[ "${health}" == healthy ]]; then
        break
    fi
    sleep 1
done

if [[ "$(docker inspect -f '{{.State.Health.Status}}' "${CONTAINER_NAME}")" != healthy ]]; then
    docker logs "${CONTAINER_NAME}" >&2
    exit 1
fi

export DATABASE_URL="postgres://${DB_USER}:${DB_PASSWORD}@127.0.0.1:${DB_PORT}/${DB_NAME}"
sqlx migrate run --source "${MIGRATIONS_DIR}"
sqlx migrate run --source "${MIGRATIONS_DIR}"

actual_tables="$(docker exec "${CONTAINER_NAME}" psql \
    -U "${DB_USER}" \
    -d "${DB_NAME}" \
    -Atqc "SELECT string_agg(tablename, ',' ORDER BY tablename) FROM pg_catalog.pg_tables WHERE schemaname = 'public' AND tablename IN ('_sqlx_migrations', 'attempt_answers', 'attempts', 'audit_logs', 'classes', 'paper_questions', 'papers', 'practice_records', 'question_answers', 'question_banks', 'question_options', 'question_revisions', 'questions', 'user_sessions', 'users');")"
expected_tables="_sqlx_migrations,attempt_answers,attempts,audit_logs,classes,paper_questions,papers,practice_records,question_answers,question_banks,question_options,question_revisions,questions,user_sessions,users"
if [[ "${actual_tables}" != "${expected_tables}" ]]; then
    printf 'error: unexpected public tables: %s\n' "${actual_tables}" >&2
    exit 1
fi

printf 'database migrations: OK\n'
