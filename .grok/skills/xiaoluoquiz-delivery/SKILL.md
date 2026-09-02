---
name: xiaoluoquiz-delivery
description: 按 xiaoluoquiz 项目约定执行功能开发、测试、Docker 构建、ECS 部署和生产数据导入。用户要求开发功能、修复问题、运行验收、发布到 ECS 或批量导入题库时使用，也可通过 /xiaoluoquiz-delivery 调用。
when-to-use: 当任务涉及 xiaoluoquiz 的代码修改、数据库迁移、前端交互、测试、Docker 发布、ECS 部署或生产题库导入时使用。
argument-hint: 要开发、测试或部署的任务
compatibility: Requires Rust, Cargo, Node.js, npm, Docker, PostgreSQL for database-backed tests, and Playwright for browser verification.
---

# xiaoluoquiz 交付流程

这个 Skill 只描述执行顺序；项目长期约束以仓库根目录的 `AGENTS.md` 和 `docs/user-story.md` 为准。

## 1. 开始前确认范围

1. 先读取 `AGENTS.md`、相关用户故事、现有实现和相关测试。
2. 判断任务属于领域/用例、HTTP API、数据库、Yew 前端、部署配置或生产数据操作。
3. 如果实现与用户故事冲突，先记录或更新用户故事，再修改代码。
4. 不读取、输出或提交密钥、密码、完整数据库连接串、会话令牌或云平台凭据。
5. 涉及生产数据时，先确认目标环境、题库 ID、导入数量和重复处理规则；默认采用只新增，不覆盖已有数据。

## 2. 按 Red-Green-Refactor 实施

### Red

- 先写一个只描述单一外部行为的失败测试。
- 运行最小范围测试，确认失败原因确实是缺少目标行为。
- 纯文档修改不伪造业务测试；部署或生产数据操作先做非破坏性预检。

### Green

- 用最小改动实现目标行为。
- 输入在边界处解析和校验，业务规则和评分由服务端执行。
- 数据库变更新增迁移文件，不修改已经应用的迁移。
- 不为无关功能顺手重构，不把敏感值写入源码、镜像或日志。

### Refactor

- 测试变绿后再整理重复逻辑、依赖方向和命名。
- 整理后重新运行受影响的最小测试。
- 再补充错误、权限、重复提交、空数据、并发和回滚边界，并重复 Red-Green-Refactor。

## 3. 根据改动类型验证

### Rust、领域和 API

按需要运行最小范围测试，交付前至少运行：

```sh
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
```

### 前端和浏览器

前端构建使用锁文件和仓库已有脚本：

```sh
npm ci
npm run build:frontend
npm run test:e2e
```

只在依赖目录缺失或锁文件发生变化时执行 `npm ci`。涉及 UI、路由、客户端状态或页面数据时，端到端测试必须按真实用户流程操作，并检查：

- 桌面和移动视口；
- 所有共享被修改状态、数据或组件的页面；
- 加载中、成功、空数据、失败和表单校验状态；
- 重复提交、刷新、返回、权限和长文本布局。

截图只能辅助检查，不能替代交互验证。若当前环境没有可用浏览器工具，使用 `npm run test:e2e` 或最接近的渲染/HTTP 检查，并在结果中明确说明限制。

### PostgreSQL 和迁移

需要真实 PostgreSQL 语义时使用 Docker PostgreSQL，不用内存数据库替代：

```sh
bash scripts/test_migrations.sh
```

本地开发数据库按 `scripts/init_db.sh` 的流程准备。迁移完成后确认：

- 干净数据库可以执行全部迁移；
- 已有数据库只向前应用新增迁移；
- 迁移重复执行不会破坏数据；
- 约束、查询和领域校验没有只依赖其中一层。

### Docker 部署契约

```sh
bash scripts/test_docker_deployment.sh
docker build --tag xiaoluoquiz:local .
```

确认运行时目录、健康检查、迁移步骤、Compose 覆盖文件和生产配置仍然一致。

## 4. ECS 部署流程

1. 先完成相关 Rust、前端、迁移和 Docker 检查。
2. 生产环境使用具体版本镜像标签，不依赖 `latest`。
3. 生产 `.env` 放在部署主机并限制为 `0600`；敏感值只通过受保护文件、环境变量或部署平台 Secret 提供。
4. 使用仓库已有的 `scripts/deploy_ecs.sh`，不要把数据库初始化复制进应用启动流程。
5. 使用本地 PostgreSQL 时同时加载 `compose.production.postgres.yaml`；数据库端口和应用端口不开放到公网，只开放反向代理需要的 `80/443`。
6. 使用 Alibaba Cloud CLI 或云助手时，只使用已确认的地域和实例参数；不请求、读取或输出 AccessKey、Secret、OAuth URL 或完整环境文件。
7. 部署后验证容器状态、数据库迁移、`/api/v1/health`、首页、登录和受影响 API；失败时保留可诊断的非敏感错误信息，不宣称部署完成。
8. 生产发布前确认数据备份和回滚策略；数据库迁移默认只向前，应用镜像使用可回滚的版本标签。

## 5. 生产题库批量导入

1. 先读取并校验 JSON 根结构、题目数量、题型、题库 ID、选项和答案引用。
2. 先查询目标 ECS 题库和题目库存，确认目标题库存在且数量符合预期。
3. 通过管理员会话调用 `POST /api/v1/admin/questions/import`，请求体使用 `{"items":[...]}`。
4. 导入采用 add-only 语义：规范化后的同题库重复题干跳过，已有题目不覆盖；新导入题目按服务端规则发布。
5. 保存并检查 `inserted`、`skipped`、`errors`；出现校验错误时确认整批没有部分写入。
6. 重复执行一次，确认不会新增重复题目。
7. 通过数据库和登录后的题目接口核对题库数量、题型、发布状态和普通用户可见字段；题面接口不得返回正确答案或解析。

## 6. 交付报告

完成时只报告已验证的事实：

- 修改了哪些文件或生产数据；
- 执行了哪些命令和测试；
- 每项结果是通过、失败还是未执行；
- 仍存在的环境限制、风险和用户需要采取的操作。

不要把密码、令牌、完整数据库 URL、私钥或完整生产环境文件放入报告。
