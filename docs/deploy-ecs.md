# ECS 部署

本文按“阿里云 ECS 云服务器 + Docker Compose”部署。应用容器负责 Axum API 和 Yew/WASM 静态文件，Caddy 负责反向代理和 HTTPS，PostgreSQL 可以使用阿里云 RDS，也可以使用 ECS 上的独立 PostgreSQL 容器。

## 1. 准备 ECS

在 ECS 上准备：

- Linux 系统。
- Docker Engine 和 Docker Compose 插件。
- 至少开放安全组入方向的 `80`、`443` 端口；不要把 `5432` 或应用的 `8000` 端口开放到公网。
- 如果使用域名和 HTTPS，把域名的 DNS A 记录指向 ECS 公网 IP。

阿里云 CLI 不需要安装在应用容器中。它适合放在本地电脑或 CI 中管理 ECS、ACR 和云助手；ECS 主机只需要 Docker。当前 CLI 已安装时，可以通过 OAuth 配置：

```bash
aliyun configure --mode OAuth
```

授权完成后，使用只读命令确认实例信息；把实际地域和实例 ID 替换为自己的值：

```bash
aliyun ecs DescribeInstances \
  --RegionId cn-hangzhou \
  --InstanceId.1 i-bpxxxxxxxxxxxxxxxx
```

## 2. 准备镜像

本仓库已经配置 GitHub Actions 工作流 `.github/workflows/publish-image.yml`，使用以下阿里云容器镜像服务 ACR 路径：

```text
crpi-628m2chfifsf3crp.cn-hangzhou.personal.cr.aliyuncs.com/xiaoluoquiz/xiaoluoquiz
```

工作流在 `master` 分支推送和形如 `v1.2.3` 的版本标签推送时构建并推送镜像，也支持手动触发时附加自定义标签。每次都会生成基于提交的 `sha-<短提交 SHA>` 标签，不生成或依赖 `latest`。

在 GitHub 仓库的 **Settings → Secrets and variables → Actions** 中配置：

- `ACR_USERNAME`：`llz00z`。
- `ACR_PASSWORD`：ACR 个人版仓库密码或访问凭证。只在 GitHub Secret 中设置，不写入仓库或聊天记录。

也可以在本地使用 GitHub CLI 设置这两个 Secret；第二条命令会从标准输入读取凭证：

```bash
gh secret set ACR_USERNAME --repo hyz123a/xiaoluoquiz --body llz00z
gh secret set ACR_PASSWORD --repo hyz123a/xiaoluoquiz
```

手动构建时使用同一镜像路径和具体标签：

```bash
docker build \
  --tag crpi-628m2chfifsf3crp.cn-hangzhou.personal.cr.aliyuncs.com/xiaoluoquiz/xiaoluoquiz:release-20260902 \
  .
docker push crpi-628m2chfifsf3crp.cn-hangzhou.personal.cr.aliyuncs.com/xiaoluoquiz/xiaoluoquiz:release-20260902
```

在 ECS 上首次拉取 ACR 镜像前，登录对应 Registry。命令会隐藏式提示输入密码，不要把密码写进命令行参数：

```bash
docker login crpi-628m2chfifsf3crp.cn-hangzhou.personal.cr.aliyuncs.com --username llz00z
```

登录成功后，部署脚本会拉取 `.env` 中 `XIAOLUOQUIZ_IMAGE` 指定的版本。GitHub Actions 使用 Secret 向工作流提供凭证，Secret 内容不会写入工作流文件。

## 3. 准备部署目录

可以使用 Git、SCP 或 CI 发布以下文件：

```text
Dockerfile
.dockerignore
compose.production.yaml
compose.production.postgres.yaml
Caddyfile
scripts/deploy_ecs.sh
.env
```

例如：

```bash
sudo mkdir -p /opt/xiaoluoquiz
sudo chown "$USER":"$USER" /opt/xiaoluoquiz
cd /opt/xiaoluoquiz
cp .env.example .env
chmod 600 .env
```

`.env` 不能提交到 Git。至少需要修改：

```dotenv
XIAOLUOQUIZ_IMAGE=crpi-628m2chfifsf3crp.cn-hangzhou.personal.cr.aliyuncs.com/xiaoluoquiz/xiaoluoquiz:20260902-1
INITIAL_PASSWORD=在 ECS 的 .env 中填入此前约定的固定初始密码
DOMAIN=quiz.example.com
```

如果先用公网 IP 做 HTTP 验证，可以临时设置：

```dotenv
DOMAIN=:80
```

正式使用域名时，把 `DOMAIN` 改为真实域名，Caddy 会根据域名配置 HTTPS。域名解析和安全组配置完成后，再通过 `https://你的域名` 访问。

## 4. 选择 PostgreSQL

### 方案 A：使用阿里云 RDS PostgreSQL

这是生产环境优先方案。`.env` 中设置 RDS 的连接地址，并关闭本机 PostgreSQL：

```dotenv
USE_LOCAL_POSTGRES=0
DATABASE_URL=postgres://app:密码@你的RDS地址:5432/xiaoluoquiz
```

如果密码包含特殊字符，需要先进行 URL 编码。RDS 数据库和账号应提前创建，安全组只允许 ECS 私网访问。

### 方案 B：使用 ECS 上的 PostgreSQL 容器

适合先快速上线或暂时不使用 RDS 的情况：

```dotenv
USE_LOCAL_POSTGRES=1
DATABASE_URL=postgres://app:一个URL安全的密码@postgres:5432/xiaoluoquiz
POSTGRES_DB=xiaoluoquiz
POSTGRES_USER=app
POSTGRES_PASSWORD=一个URL安全的密码
```

`DATABASE_URL` 中的密码必须和 `POSTGRES_PASSWORD` 一致。PostgreSQL 数据保存于 Docker volume `postgres_data`，不会随着容器重建而删除；仍然需要另外配置备份。

## 5. 执行部署

确保脚本可执行：

```bash
chmod +x scripts/deploy_ecs.sh
```

使用 ACR 镜像部署：

```bash
./scripts/deploy_ecs.sh
```

脚本会依次：

1. 校验 `.env` 和 Compose 配置。
2. 拉取指定版本的应用、Caddy 和可选 PostgreSQL 镜像。
3. 使用同一个应用镜像中的 SQLx CLI 执行数据库迁移。
4. 启动应用和反向代理。
5. 输出容器状态。

首次使用本地构建镜像时，可以在 ECS 上执行：

```bash
BUILD_IMAGE=1 ./scripts/deploy_ecs.sh
```

通常建议在本地或 CI 构建并推送 ACR，ECS 只负责拉取和运行。

## 6. 更新和回滚

更新时只需要修改 `.env` 中的镜像标签，然后重新执行部署脚本：

```dotenv
XIAOLUOQUIZ_IMAGE=crpi-628m2chfifsf3crp.cn-hangzhou.personal.cr.aliyuncs.com/xiaoluoquiz/xiaoluoquiz:20260902-2
```

```bash
./scripts/deploy_ecs.sh
```

回滚时换回上一个镜像标签并重新执行脚本。数据库迁移默认只向前演进，执行涉及结构变化的版本更新前应先备份数据库，并确认应用版本兼容。

## 7. 检查运行状态

```bash
docker compose --env-file .env -f compose.production.yaml ps
docker compose --env-file .env -f compose.production.yaml logs --tail=100 app
```

如果使用本地 PostgreSQL，命令需要追加覆盖文件：

```bash
docker compose \
  --env-file .env \
  -f compose.production.yaml \
  -f compose.production.postgres.yaml \
  ps
```

应用容器内置健康检查，检查接口为：

```text
/api/v1/health
```

首次登录后应立即修改部署配置中的初始密码对应的管理员密码。生产环境不要把 `.env`、数据库连接串或管理员密码放入镜像和 Git 仓库。
