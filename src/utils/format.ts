function pad(value: number): string {
  return String(value).padStart(2, '0')
}

export function formatDuration(totalSeconds: number | null): string {
  if (totalSeconds === null || !Number.isFinite(totalSeconds) || totalSeconds < 0) {
    return '--:--:--'
  }

  const seconds = Math.floor(totalSeconds)
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  const remainingSeconds = seconds % 60

  return `${pad(hours)}:${pad(minutes)}:${pad(remainingSeconds)}`
}

export function formatCount(value: number | null): string {
  if (value === null || !Number.isFinite(value)) {
    return '--'
  }

  const absolute = Math.abs(value)
  if (absolute >= 100_000_000) {
    return `${trimDecimal(value / 100_000_000)}亿`
  }
  if (absolute >= 10_000) {
    return `${trimDecimal(value / 10_000)}万`
  }

  return new Intl.NumberFormat('zh-CN').format(value)
}

function trimDecimal(value: number): string {
  return value
    .toFixed(1)
    .replace(/\.0$/, '')
    .replace(/(\.\d)0$/, '$1')
}

export function formatClock(timestamp: number | null): string {
  if (timestamp === null) {
    return '--:--:--'
  }

  return new Intl.DateTimeFormat('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  }).format(timestamp)
}

export function formatArea(parentAreaName: string, areaName: string): string {
  if (parentAreaName && areaName && parentAreaName !== areaName) {
    return `${parentAreaName} / ${areaName}`
  }

  return areaName || parentAreaName || '--'
}
