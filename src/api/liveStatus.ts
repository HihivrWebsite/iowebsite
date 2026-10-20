export interface LiveStatusSnapshot {
  uid: number
  roomId: number
  uname: string
  title: string
  isLive: boolean
  durationSec: number
  online: number | null
  attention: number | null
  areaName: string
  parentAreaName: string
  updatedAt: string | null
}

interface LiveStatusEnvelope {
  code: number
  message: string
  data: Record<string, unknown>
}

const API_PATH = '/api/live-status'
const QUERY = 'uid=85996232&room_id=1890510937'

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function toNumber(value: unknown): number | null {
  if (value === null || value === undefined || value === '') {
    return null
  }

  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : null
}

function toText(value: unknown): string {
  if (typeof value === 'string') {
    return value.trim()
  }

  if (typeof value === 'number' && Number.isFinite(value)) {
    return String(value)
  }

  return ''
}

function parseEnvelope(payload: unknown): LiveStatusEnvelope {
  if (!isRecord(payload)) {
    throw new Error('直播状态接口返回了无法识别的数据')
  }

  const code = toNumber(payload.code)
  const data = payload.data

  if (code !== null && code !== 0 && code !== 200) {
    throw new Error(toText(payload.message) || `直播状态接口异常（${code}）`)
  }

  if (!isRecord(data)) {
    throw new Error(toText(payload.message) || '直播状态接口缺少 data 字段')
  }

  return {
    code: code ?? 0,
    message: toText(payload.message),
    data,
  }
}

function deriveDuration(
  rawDuration: unknown,
  rawLiveTime: unknown,
  isLive: boolean,
): number {
  const duration = toNumber(rawDuration)
  if (duration !== null) {
    return Math.max(0, Math.floor(duration))
  }

  if (!isLive) {
    return 0
  }

  const rawTime = toNumber(rawLiveTime)
  if (rawTime === null) {
    return 0
  }

  const startAt = rawTime < 100_000_000_000 ? rawTime * 1000 : rawTime
  const elapsed = Date.now() - startAt
  return elapsed > 0 && elapsed < 7 * 24 * 60 * 60 * 1000
    ? Math.floor(elapsed / 1000)
    : 0
}

function normalize(payload: unknown): LiveStatusSnapshot {
  const { data } = parseEnvelope(payload)
  const liveValue = toNumber(data.live_status)
  const isLive = liveValue === 1 || data.live_status === true || data.live_status === 'true'
  const areaName = toText(data.area_name)
  const parentAreaName = toText(data.parent_area_name)

  return {
    uid: toNumber(data.uid) ?? 85996232,
    roomId: toNumber(data.room_id) ?? 1890510937,
    uname: toText(data.uname) || '咿喔io',
    title: toText(data.title) || (isLive ? '直播中' : '暂无直播标题'),
    isLive,
    durationSec: deriveDuration(data.duration_sec, data.live_time, isLive),
    online: toNumber(data.online),
    attention: toNumber(data.attention),
    areaName,
    parentAreaName,
    updatedAt: toText(data.updated_at) || null,
  }
}

export async function fetchLiveStatus(signal: AbortSignal): Promise<LiveStatusSnapshot> {
  let response: Response

  try {
    response = await fetch(`${API_PATH}?${QUERY}`, {
      method: 'GET',
      headers: {
        Accept: 'application/json',
      },
      cache: 'no-store',
      credentials: 'same-origin',
      signal,
    })
  } catch (error) {
    if (error instanceof DOMException && error.name === 'AbortError') {
      throw error
    }
    throw new Error('暂时无法连接直播状态服务')
  }

  if (!response.ok) {
    throw new Error(`直播状态服务响应异常（HTTP ${response.status}）`)
  }

  let payload: unknown
  try {
    payload = await response.json()
  } catch {
    throw new Error('直播状态服务返回了无效 JSON')
  }

  return normalize(payload)
}
