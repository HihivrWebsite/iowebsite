<script setup lang="ts">
import ActionButtons from './components/ActionButtons.vue'
import JellyCursor from './components/JellyCursor.vue'
import LiveStatusCard from './components/LiveStatusCard.vue'
import SiteHeader from './components/SiteHeader.vue'
import { useLiveStatus } from './composables/useLiveStatus'

const {
  snapshot,
  loading,
  error,
  stale,
  lastSuccessAt,
  displayState,
} = useLiveStatus()
</script>

<template>
  <div class="site-shell">
    <div class="ambient ambient-one" aria-hidden="true"></div>
    <div class="ambient ambient-two" aria-hidden="true"></div>

    <SiteHeader />

    <main id="top" class="page-main">
      <section class="hero entrance entrance-hero" aria-labelledby="page-title">
        <h1 id="page-title">你好咿喔</h1>
        <p class="hero-subtitle">咿喔io粉丝站</p>
      </section>

      <LiveStatusCard
        :snapshot="snapshot"
        :loading="loading"
        :error="error"
        :stale="stale"
        :last-success-at="lastSuccessAt"
        :display-state="displayState"
      />

      <ActionButtons />
    </main>

    <JellyCursor />
  </div>
</template>
