<script setup lang="ts">
import type { LiveStatusSnapshot } from '../api/liveStatus'
import {
  formatArea,
  formatClock,
  formatCount,
  formatDuration,
} from '../utils/format'

defineProps<{
  snapshot: LiveStatusSnapshot | null
  loading: boolean
  error: string | null
  stale: boolean
  lastSuccessAt: number | null
  displayState: 'loading' | 'error' | 'stale' | 'ready'
}>()
</script>

<template>
  <section
    class="live-card glass-panel entrance entrance-card"
    :class="`state-${displayState}`"
    aria-labelledby="live-card-title"
    aria-live="polite"
  >
    <div class="live-card__header">
      <div>
        <p class="section-label">LIVE STATUS</p>
        <h2 id="live-card-title">直播状态</h2>
      </div>

      <div class="sync-state" :class="{ 'is-busy': loading, 'is-warning': stale }">
        <span v-if="loading" class="sync-spinner" aria-hidden="true"></span>
        <span v-else class="sync-dot" aria-hidden="true"></span>
        <span>
          {{ loading ? '同步中' : stale ? '缓存快照' : '实时更新' }}
        </span>
      </div>
    </div>

    <div v-if="!snapshot && loading" class="state-message loading-message">
      <span class="skeleton skeleton-line skeleton-wide"></span>
      <span class="skeleton skeleton-line"></span>
      <span class="skeleton skeleton-line skeleton-short"></span>
      <p>正在连接直播状态服务…</p>
    </div>

    <div
      v-else-if="!snapshot && error"
      class="state-message error-message"
      role="alert"
    >
      <span class="error-mark" aria-hidden="true">!</span>
      <div>
        <strong>暂时拿不到直播状态</strong>
        <p>{{ error }}</p>
        <p class="retry-hint">页面会在后台恢复后自动重试。</p>
      </div>
    </div>

    <template v-else-if="snapshot">
      <div class="streamer-row">
        <div class="streamer-identity">
          <p class="streamer-name">{{ snapshot.uname }}</p>
          <p class="stream-title">{{ snapshot.title }}</p>
        </div>

        <div
          class="live-badge"
          :class="{ 'is-live': snapshot.isLive, 'is-offline': !snapshot.isLive }"
        >
          <span aria-hidden="true"></span>
          {{ snapshot.isLive ? '直播中' : '未开播' }}
        </div>
      </div>

      <dl class="status-grid">
        <div class="status-item status-duration">
          <dt>直播时长</dt>
          <dd>
            {{
              snapshot.isLive
                ? formatDuration(snapshot.durationSec)
                : '--:--:--'
            }}
          </dd>
        </div>
        <div class="status-item">
          <dt>人气</dt>
          <dd>{{ formatCount(snapshot.online) }}</dd>
        </div>
        <div class="status-item">
          <dt>关注</dt>
          <dd>{{ formatCount(snapshot.attention) }}</dd>
        </div>
        <div class="status-item">
          <dt>分区</dt>
          <dd>
            {{ formatArea(snapshot.parentAreaName, snapshot.areaName) }}
          </dd>
        </div>
        <div class="status-item">
          <dt>房间号</dt>
          <dd>{{ snapshot.roomId }}</dd>
        </div>
        <div class="status-item">
          <dt>UID</dt>
          <dd>{{ snapshot.uid }}</dd>
        </div>
      </dl>

      <p v-if="stale || error" class="snapshot-notice" :role="error ? 'status' : undefined">
        <template v-if="error">
          最新刷新失败：{{ error }}；正在显示
        </template>
        <template v-else>数据超过更新时间，正在显示</template>
        {{ formatClock(lastSuccessAt) }} 的成功快照。
      </p>
      <p v-else class="update-time">
        最近成功更新 {{ formatClock(lastSuccessAt) }}
      </p>
    </template>
  </section>
</template>
