# 架构总览

## 目标

本项目是面向 B 站主播/粉丝的直播状态展示站点。请求链路尽量短：浏览器只访问本项目的 Rust 服务，由服务端统一访问 B 站公开接口、做字段归一化和短时缓存，再把稳定的 JSON 返回给 Vue 页面。

## 组件

```text
Browser
  └─ Vue SPA (dist/)
       └─ GET /api/live-status
            └─ Rust server (PORT=2993)
                 ├─ 20 秒成功缓存 / 失败退避快照
                 └─ B 站公开接口
                      ├─ room/v1/Room/get_status_info_by_uids
                      └─ room/v1/Room/room_init（必要时补全）
```

- **前端**：Vue 单页应用负责状态展示、错误/轮播状态呈现，以及封面、关键帧等媒体资源的展示。构建产物放入 `dist/`，由 Rust 静态文件服务托管，并启用 SPA fallback。
- **后端**：Rust 服务暴露 `GET /healthz`、`GET /api/version` 和 `GET /api/live-status`。默认监听 `2993`，可通过 `PORT` 和 `STATIC_DIR` 覆盖配置。
- **资源**：图标等图片在开发阶段从图床下载，使用本地工具压缩、转码后提交到 `src/assets/`。运行时只使用仓库内资源，不把图床当作可用性依赖。
- **更新脚本**：`scripts/update-start.sh` 是一键更新启动入口，负责拉取/安装依赖、构建前端和 Rust 服务，并以前台方式启动；脚本不执行 Git 提交或推送。

## 请求与数据流

1. Vue 页面按固定间隔调用 `/api/live-status?uid=...&room_id=...`。
2. Rust 服务校验参数，合并同一 `uid+room_id` 的并发刷新，单请求上游预算为 5 秒。
3. 服务优先读取 20 秒内的成功缓存；缓存过期时访问 B 站接口并归一化字段。
4. B 站接口失败时，在退避窗口内返回最近一次成功快照，并在响应中保留 `updated_at` 供前端展示数据新鲜度。
5. 完全没有快照时返回非零 `code` 和明确的 `message`，前端显示可恢复的错误状态。

字段定义和示例见 [docs/api/live-status.md](../api/live-status.md)。

## 可靠性与安全边界

- 只转发公开的 B 站接口，不携带 Cookie、Authorization 或其他用户凭据。
- 请求头仅包含必要的浏览器 `User-Agent`、`Referer`、`Accept`，避免泄漏本地环境信息。
- 使用超时、缓存、退避和并发去重降低上游限流与抖动对页面的影响。
- 静态目录路径由服务端校验，避免目录穿越；API 返回统一信封，前端不直接依赖上游响应结构。

## 部署

推荐使用 `systemd/io-live-status.service` 运行 Rust 服务，工作目录指向项目根目录，设置 `PORT=2993`（如需变更请同步文档）。服务只负责托管 `dist/` 和 API，更新代码和构建可复用 `scripts/update-start.sh`，由用户确认后执行。

