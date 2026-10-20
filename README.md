# IO Live Status

面向 B 站直播间状态展示的轻量站点，前端使用 Vue，后端使用 Rust。后端代理 B 站公开接口并提供统一的 JSON 信封，前端由 Rust 服务托管，默认监听 `2993` 端口。

## 环境要求

部署前请准备以下依赖：

- Git：用于克隆仓库及执行更新脚本的 `git pull --ff-only`。
- Node.js：建议使用 Node.js 22 LTS（项目依赖要求 Node.js 20.19+ 或 22.12+）。
- npm：随 Node.js 安装，用于安装前端依赖和执行构建。
- Rust/Cargo：`Cargo.toml` 要求 Rust 1.88 或更高版本，推荐通过 [rustup](https://rustup.rs/) 安装。
- curl：用于部署后的 HTTP 验证。

可先检查工具是否可用：

```bash
git --version
node --version
npm --version
rustc --version
cargo --version
curl --version
```

Ubuntu 可参考以下方式安装基础工具；Rust 版本过旧时请使用 rustup 升级：

```bash
sudo apt update
sudo apt install -y git curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Node.js 可通过系统包管理器、版本管理器或官方安装方式部署，安装后确认版本满足上述要求即可。

## 首次部署

以下命令以仓库默认分支和 GitHub 组织仓库为例：

```bash
git clone https://github.com/HihivrWebsite/iowebsite.git
cd iowebsite
chmod +x scripts/update-start.sh
PORT=2993 STATIC_DIR="$PWD/dist" ./scripts/update-start.sh
```

该命令默认以后台方式执行以下流程：

1. 使用 `git pull --ff-only` 自动更新代码；
2. 执行 `npm install` 安装前端依赖；
3. 执行 `npm run build` 构建前端并生成 `dist/`；
4. 执行 `cargo build --release` 构建 Rust 后端；
5. 安全停止旧进程并后台启动服务。

因此，后续每次执行同一命令，都会先自动更新，再重新构建并以后台方式启动服务：

```bash
PORT=2993 STATIC_DIR="$PWD/dist" ./scripts/update-start.sh
```

后台模式的 PID 写入 `.run/iowebsite.pid`，日志写入 `.run/iowebsite.log`。服务默认使用 `127.0.0.1:2993` 监听，`STATIC_DIR` 默认为仓库根目录下的 `dist/`。

> 注意：`git pull --ff-only` 要求本地没有与远端冲突的提交。若更新失败，请先人工审查并处理本地变更，不要强制覆盖远端或本地修改。

## 前台与跳过更新

需要由终端、容器或 systemd 接管进程时，使用前台模式：

```bash
PORT=2993 STATIC_DIR="$PWD/dist" ./scripts/update-start.sh --foreground
```

只重新构建和启动、明确跳过 `git pull` 时，使用 `--skip-update`；该选项仍会执行 npm 和 Rust 构建：

```bash
PORT=2993 STATIC_DIR="$PWD/dist" ./scripts/update-start.sh --skip-update
```

前台模式也可与跳过更新组合：

```bash
PORT=2993 STATIC_DIR="$PWD/dist" ./scripts/update-start.sh --skip-update --foreground
```

查看脚本完整参数：

```bash
./scripts/update-start.sh --help
```

## 部署验证

服务启动后，可从浏览器访问：

- 站点首页：<http://127.0.0.1:2993/>
- 健康检查：<http://127.0.0.1:2993/healthz>
- 版本信息：<http://127.0.0.1:2993/api/version>
- 直播状态接口：<http://127.0.0.1:2993/api/live-status>

也可使用命令验证：

```bash
curl -fsS http://127.0.0.1:2993/healthz
curl -fsS http://127.0.0.1:2993/api/version
curl -fsSI http://127.0.0.1:2993/
ss -ltnp | grep ':2993 '
```

从服务器外部访问时，将 `127.0.0.1` 替换为服务器 IP 或域名，例如 `http://SERVER_IP:2993/`。

## 进程管理：二选一

同一端口只能选择以下一种托管方式，请勿让脚本后台模式和 systemd 同时启动服务：

1. **脚本后台模式（简单部署）**：

   ```bash
   PORT=2993 STATIC_DIR="$PWD/dist" ./scripts/update-start.sh
   ```

   适合手动维护；每次执行都会自动更新、构建、替换旧进程并后台启动。

2. **systemd 模式（服务器长期运行）**：

   按 [systemd 部署说明](systemd/README.md) 安装并启动 `io-live-status.service`，由 systemd 保证异常退出后自动重启。构建更新时使用 `--skip-update` 或由部署用户单独执行更新，不要同时运行脚本后台模式。

若停用脚本后台模式再切换到 systemd，或反向切换，请先确认 `.run/iowebsite.pid` 中的旧进程和 systemd 服务状态，避免两个进程竞争 `2993` 端口。

## 防火墙

仅本机访问时无需开放防火墙。需要从外部访问 `2993` 时，应同时检查主机防火墙、云安全组或其他网络防火墙，并只开放 TCP `2993`。

使用 UFW 时可执行：

```bash
sudo ufw allow 2993/tcp
sudo ufw reload
sudo ufw status
```

不使用 UFW 时，请按实际防火墙工具开放 TCP `2993`，并在外部执行以下验证（将 `SERVER_IP` 替换为服务器地址）：

```bash
curl -fsS "http://SERVER_IP:2993/healthz"
```

## 开发说明

- 前端：Vue 单页应用，构建产物由 Rust 服务托管。
- 后端：Rust HTTP 服务，提供健康检查、版本查询和 `/api/live-status` 代理。
- 版本：根目录 `VERSION` 为 `1.0.0`，接口和更新脚本均以该版本为准。
- 图标：从图床下载原始图片后，使用本地工具压缩/转换并提交到仓库，运行时不再依赖图床。

## API

详见 [docs/api/live-status.md](docs/api/live-status.md)。所有接口使用 `{code,message,data}` 信封；直播状态字段来自 B 站接口的代理与归一化结果。

## 文档

- [架构总览](docs/architecture/overview.md)
- [粉丝站点功能](docs/features/fan-site.md)
- [变更记录](docs/changes/2026-10-21-initial-implementation.md)
- [systemd 部署说明](systemd/README.md)

## 版本与发布

更新前请确认工作区干净并审查变更。提交时使用约定标题：

```text
2026-10-21-14:30 PMA213X 完成咿喔io粉丝站部署文档并上传
```
