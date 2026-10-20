<script setup lang="ts">
import logoUrl from '../assets/logo.png'
</script>

<template>
  <header class="site-header entrance entrance-header">
    <a class="brand-lockup" href="#top" aria-label="咿喔io粉丝站，返回页面顶部">
      <span class="brand-mark" aria-hidden="true">
        <img :src="logoUrl" width="72" height="72" alt="" />
      </span>
      <span class="brand-name">咿喔io粉丝站</span>
    </a>

    <nav class="site-nav" aria-label="主要导航">
      <a class="nav-link is-home" href="#top">主页</a>
      <a
        class="nav-link"
        href="https://live.bilibili.com/1890510937"
        target="_blank"
        rel="noopener noreferrer"
      >
        进入直播间
      </a>
      <a
        class="nav-link"
        href="https://space.bilibili.com/85996232"
        target="_blank"
        rel="noopener noreferrer"
      >
        关注主播
      </a>
      <a
        class="nav-link"
        href="https://dc.yioio.top"
        target="_blank"
        rel="noopener noreferrer"
      >
        进入维阿PSP斗虫榜
      </a>
    </nav>
  </header>
</template>
