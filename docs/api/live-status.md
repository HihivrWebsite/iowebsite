# Live Status API

## 通用信封

所有业务接口返回：

```json
{
  "code": 0,
  "message": "ok",
  "data": {}
}
```

`code=0` 表示请求成功；失败时 `data` 可能为 `null`，`message` 必须可读且不应泄漏上游原始响应。

## GET /api/live-status

查询参数：

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `uid` | 数字字符串 | 是 | B 站用户/主播 UID |
| `room_id` | 数字字符串 | 是 | 直播间 ID |

示例：

```text
GET /api/live-status?uid=85996232&room_id=1890510937
```

### data 字段

响应 `data` 固定包含以下字段：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `uid` | number | 请求的主播 UID |
| `room_id` | number | 请求的直播间 ID |
| `uname` | string | 主播昵称 |
| `title` | string | 直播间标题 |
| `live_status` | number | `0=未开播`、`1=直播中`、`2=轮播` |
| `live_time` | string/null | 开播时间的可读表示 |
| `duration_sec` | number | 本次直播持续秒数；仅直播中计算，其他状态为 `0` |
| `online` | number | 当前在线人数（上游返回时） |
| `attention` | number | 关注数（上游返回时） |
| `area_name` | string | 分区名称 |
| `parent_area_name` | string | 父分区名称 |
| `cover` | string/null | 封面 URL |
| `keyframe` | string/null | 关键帧 URL |
| `updated_at` | string | 服务端最近一次成功数据时间（ISO 8601） |

上游接口优先请求：

```text
https://api.live.bilibili.com/room/v1/Room/get_status_info_by_uids?uids%5B%5D={uid}
```

必要时补充 `room/v1/Room/room_init?id={room_id}`，再按 `uid+room_id` 合并结果。上游字段名可能随时间变化，后端负责把时间戳、时间字符串、字符串数值等输入统一转换为上表类型。

### 降级策略

1. 单次上游总预算为 5 秒；超过预算按失败处理。
2. 成功结果缓存 20 秒；缓存期内直接返回，不重复请求上游。
3. 上游失败时，在退避窗口内返回最近一次成功快照，并保留其 `updated_at`。
4. 同一 `uid+room_id` 的并发请求只触发一次刷新，减少上游压力。
5. 没有任何历史快照时返回非零 `code`，`message` 说明是上游超时、状态码异常还是响应解析失败。

`duration_sec` 只在 `live_status=1` 且能够得到有效开播时间时计算；无法解析时为 `0`。B 站的极值时间戳 `-62170012800` 被视为无开播时间并归零。

## GET /healthz

返回服务存活状态，`data` 至少包含 `status`、`version` 和 `uptime`。该接口不应访问上游接口，适合探针和部署检查。

## GET /api/version

返回当前服务版本。版本值以根目录 `VERSION` 为准，当前为 `1.0.0`。

