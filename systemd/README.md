# systemd 部署说明

本目录的脚本和服务文件不会自动执行任何 `sudo`、`systemctl` 或 Git 操作；权限提升仅由管理员在审核后手工执行。

## 1. 准备部署用户和目录

推荐把仓库部署到 `/opt/iowebsite`，由专用无登录用户运行：

```bash
sudo useradd --system --home-dir /opt/iowebsite --shell /usr/sbin/nologin iowebsite
sudo chown -R iowebsite:iowebsite /opt/iowebsite
```

`server/target/release/iowebsite` 和前端 `dist/index.html` 必须存在。可由部署用户执行：

```bash
cd /opt/iowebsite
./scripts/update-start.sh --background
```

脚本默认端口为 2993，默认静态目录为 `/opt/iowebsite/dist`，不会调用 `sudo`，且只停止 PID 文件中经 `/proc/PID/exe` 验证属于本仓库二进制的进程。

## 2. 安装服务

管理员审核后执行：

```bash
sudo install -m 0644 systemd/io-live-status.service /etc/systemd/system/io-live-status.service
sudoedit /etc/iowebsite.env
```

`/etc/iowebsite.env` 可使用：

```dotenv
PORT=2993
STATIC_DIR=/opt/iowebsite/dist
```

随后：

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now io-live-status
sudo systemctl restart io-live-status
```

若仓库或运行用户不同，先修改 unit 中的 `User`、`Group`、`WorkingDirectory`、`Environment=STATIC_DIR` 和 `ExecStart`，再安装。

## 3. 验证

```bash
systemctl --no-pager status io-live-status
curl -fsS http://127.0.0.1:2993/healthz
curl -fsS 'http://127.0.0.1:2993/api/live-status?uid=85996232&room_id=1890510937'
```

## 4. 两种启动方式

- `./scripts/update-start.sh --background`：脚本更新、构建、停止旧进程并后台启动，PID 写入 `.run/iowebsite.pid`，日志写入 `.run/iowebsite.log`。
- `./scripts/update-start.sh --foreground`：完成更新和构建、安全停止旧进程后以前台方式 `exec`，适合 systemd 或容器接管。

systemd 管理时，日常构建更新建议由部署用户执行：

```bash
cd /opt/iowebsite
./scripts/update-start.sh --skip-update --foreground
```

若该前台任务由 systemd 启动，不要同时再直接运行后台脚本，避免两个进程竞争同一端口。
