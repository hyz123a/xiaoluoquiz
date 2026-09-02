#!/usr/bin/env bash
set -Eeuo pipefail

readonly ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
readonly REQUIRED_FILES=(
    "Dockerfile"
    ".dockerignore"
    "compose.production.yaml"
    "compose.production.postgres.yaml"
    "Caddyfile"
    ".env.example"
    "scripts/deploy_ecs.sh"
    "docs/deploy-ecs.md"
)

for relative_path in "${REQUIRED_FILES[@]}"; do
    if [[ ! -f "${ROOT_DIR}/${relative_path}" ]]; then
        printf 'error: required deployment file not found: %s\n' "${relative_path}" >&2
        exit 1
    fi
done

if [[ ! -x "${ROOT_DIR}/scripts/deploy_ecs.sh" ]]; then
    printf 'error: deploy script is not executable\n' >&2
    exit 1
fi

if ! grep -q 'STATIC_DIR=/app/dist' "${ROOT_DIR}/Dockerfile"; then
    printf 'error: Dockerfile must configure the container static directory\n' >&2
    exit 1
fi

if ! grep -q 'migrate' "${ROOT_DIR}/compose.production.yaml"; then
    printf 'error: production Compose configuration must define an explicit migration step\n' >&2
    exit 1
fi

readonly ENV_FILE="$(mktemp)"
cleanup() {
    rm -f "${ENV_FILE}"
}
trap cleanup EXIT

cat >"${ENV_FILE}" <<'EOF'
XIAOLUOQUIZ_IMAGE=xiaoluoquiz:test
DOMAIN=quiz.example.com
DATABASE_URL=postgres://app:secret@postgres:5432/xiaoluoquiz
INITIAL_PASSWORD=ChangeThisPassword123!
INITIAL_ADMIN_USERNAME=admin
INITIAL_ADMIN_DISPLAY_NAME=系统管理员
POSTGRES_DB=xiaoluoquiz
POSTGRES_USER=app
POSTGRES_PASSWORD=secret
EOF

cd "${ROOT_DIR}"
docker compose --env-file "${ENV_FILE}" -f compose.production.yaml config >/dev/null
docker compose --env-file "${ENV_FILE}" -f compose.production.yaml -f compose.production.postgres.yaml config >/dev/null
printf 'docker deployment configuration: OK\n'
