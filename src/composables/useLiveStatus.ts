import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { fetchLiveStatus, type LiveStatusSnapshot } from '../api/liveStatus'

const POLL_INTERVAL_MS = 30_000
const STALE_AFTER_MS = 75_000
const CLOCK_TICK_MS = 5_000

function readableError(error: unknown): string {
  if (error instanceof Error && error.message) {
    return error.message
  }

  return '直播状态刷新失败'
}

export function useLiveStatus() {
  const snapshot = ref<LiveStatusSnapshot | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)
  const lastSuccessAt = ref<number | null>(null)
  const clock = ref(Date.now())

  let pollTimer: number | null = null
  let clockTimer: number | null = null
  let controller: AbortController | null = null
  let inFlight = false
  let pendingRefresh = false
  let unmounted = false

  const stale = computed(() => {
    if (!snapshot.value || lastSuccessAt.value === null) {
      return false
    }

    return (
      error.value !== null ||
      clock.value - lastSuccessAt.value >= STALE_AFTER_MS
    )
  })

  const displayState = computed<'loading' | 'error' | 'stale' | 'ready'>(() => {
    if (!snapshot.value) {
      return loading.value ? 'loading' : 'error'
    }
    if (stale.value) {
      return 'stale'
    }

    return 'ready'
  })

  function stopPolling(): void {
    if (pollTimer !== null) {
      window.clearTimeout(pollTimer)
      pollTimer = null
    }
  }

  function scheduleNext(): void {
    stopPolling()

    if (unmounted || document.hidden) {
      return
    }

    pollTimer = window.setTimeout(() => {
      pollTimer = null
      void refresh()
    }, POLL_INTERVAL_MS)
  }

  async function refresh(): Promise<void> {
    if (unmounted) {
      return
    }

    if (inFlight) {
      if (!document.hidden) {
        pendingRefresh = true
      }
      return
    }

    inFlight = true
    loading.value = true
    clock.value = Date.now()
    controller = new AbortController()

    try {
      const nextSnapshot = await fetchLiveStatus(controller.signal)
      snapshot.value = nextSnapshot
      lastSuccessAt.value = Date.now()
      clock.value = lastSuccessAt.value
      error.value = null
    } catch (caught) {
      if (
        caught instanceof DOMException &&
        caught.name === 'AbortError'
      ) {
        return
      }
      error.value = readableError(caught)
    } finally {
      controller = null
      inFlight = false
      loading.value = false

      if (unmounted) {
        return
      }

      if (pendingRefresh && !document.hidden) {
        pendingRefresh = false
        void refresh()
      } else if (!document.hidden) {
        scheduleNext()
      }
    }
  }

  function handleVisibilityChange(): void {
    clock.value = Date.now()

    if (document.hidden) {
      stopPolling()
      controller?.abort()
      return
    }

    if (inFlight) {
      controller?.abort()
      pendingRefresh = true
      return
    }

    void refresh()
  }

  onMounted(() => {
    document.addEventListener('visibilitychange', handleVisibilityChange)
    clockTimer = window.setInterval(() => {
      if (!document.hidden) {
        clock.value = Date.now()
      }
    }, CLOCK_TICK_MS)
    void refresh()
  })

  onBeforeUnmount(() => {
    unmounted = true
    stopPolling()
    document.removeEventListener('visibilitychange', handleVisibilityChange)

    if (clockTimer !== null) {
      window.clearInterval(clockTimer)
      clockTimer = null
    }

    controller?.abort()
  })

  return {
    snapshot,
    loading,
    error,
    stale,
    lastSuccessAt,
    displayState,
  }
}
