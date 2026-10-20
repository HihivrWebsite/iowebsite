<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'

const ENABLED_CLASS = 'custom-cursor-enabled'
const layerRef = ref<HTMLElement | null>(null)
const dotRef = ref<HTMLElement | null>(null)
const ringRef = ref<HTMLElement | null>(null)
const enabled = ref(false)
const visible = ref(false)

let pointerQuery: MediaQueryList | null = null
let motionQuery: MediaQueryList | null = null
let animationFrame = 0
let lastFrame = 0
let pointerSeen = false
let pointerX = window.innerWidth / 2
let pointerY = window.innerHeight / 2
let ringX = pointerX
let ringY = pointerY
let ringVelocityX = 0
let ringVelocityY = 0
let angle = 0
let stretch = 1
let stretchVelocity = 0
let unmounted = false

function canEnable(): boolean {
  return Boolean(
    pointerQuery?.matches &&
      motionQuery &&
      !motionQuery.matches &&
      !unmounted,
  )
}

function activate(): void {
  if (enabled.value || !canEnable()) {
    return
  }

  enabled.value = true
  document.documentElement.classList.add(ENABLED_CLASS)
  startAnimation()
}

function deactivate(): void {
  enabled.value = false
  visible.value = false
  document.documentElement.classList.remove(ENABLED_CLASS)
  stopAnimation()
}

function syncCapability(): void {
  if (canEnable()) {
    activate()
  } else {
    deactivate()
  }
}

function handlePointerMove(event: PointerEvent): void {
  if (event.pointerType === 'touch' || !enabled.value) {
    return
  }

  pointerX = event.clientX
  pointerY = event.clientY

  if (!pointerSeen) {
    ringX = pointerX
    ringY = pointerY
    pointerSeen = true
  }

  visible.value = true
}

function hideWhenPointerLeaves(event: PointerEvent): void {
  if (event.relatedTarget === null) {
    visible.value = false
  }
}

function hideOnBlur(): void {
  visible.value = false
}

function startAnimation(): void {
  if (animationFrame !== 0 || unmounted) {
    return
  }

  lastFrame = 0
  animationFrame = requestAnimationFrame(animate)
}

function stopAnimation(): void {
  if (animationFrame !== 0) {
    cancelAnimationFrame(animationFrame)
    animationFrame = 0
  }
}

function animate(timestamp: number): void {
  if (unmounted || !enabled.value) {
    animationFrame = 0
    return
  }

  const frameDelta = lastFrame === 0 ? 16.667 : timestamp - lastFrame
  lastFrame = timestamp
  const step = Math.min(Math.max(frameDelta / 16.667, 0.25), 2)

  ringVelocityX += (pointerX - ringX) * 0.18 * step
  ringVelocityY += (pointerY - ringY) * 0.18 * step
  ringVelocityX *= Math.pow(0.76, step)
  ringVelocityY *= Math.pow(0.76, step)
  ringX += ringVelocityX * step
  ringY += ringVelocityY * step

  const speed = Math.hypot(ringVelocityX, ringVelocityY)
  if (speed > 0.08) {
    angle = Math.atan2(ringVelocityY, ringVelocityX)
  }

  const targetStretch = Math.min(1 + speed * 0.018, 1.88)
  stretchVelocity += (targetStretch - stretch) * 0.16 * step
  stretchVelocity *= Math.pow(0.81, step)
  stretch += stretchVelocity * step
  stretch = Math.min(Math.max(stretch, 0.7), 1.94)
  const squash = 1 / Math.sqrt(stretch)

  if (dotRef.value) {
    dotRef.value.style.transform =
      `translate3d(${pointerX}px, ${pointerY}px, 0) ` +
      'translate(-50%, -50%)'
  }

  if (ringRef.value) {
    ringRef.value.style.transform =
      `translate3d(${ringX}px, ${ringY}px, 0) ` +
      'translate(-50%, -50%) ' +
      `rotate(${angle}rad) scale(${stretch.toFixed(4)}, ${squash.toFixed(4)})`
  }

  animationFrame = requestAnimationFrame(animate)
}

onMounted(() => {
  pointerQuery = window.matchMedia('(hover: hover) and (pointer: fine)')
  motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)')
  pointerQuery.addEventListener('change', syncCapability)
  motionQuery.addEventListener('change', syncCapability)
  window.addEventListener('pointermove', handlePointerMove, { passive: true })
  document.addEventListener('pointerout', hideWhenPointerLeaves)
  window.addEventListener('blur', hideOnBlur)
  syncCapability()
})

onBeforeUnmount(() => {
  unmounted = true
  deactivate()
  pointerQuery?.removeEventListener('change', syncCapability)
  motionQuery?.removeEventListener('change', syncCapability)
  window.removeEventListener('pointermove', handlePointerMove)
  document.removeEventListener('pointerout', hideWhenPointerLeaves)
  window.removeEventListener('blur', hideOnBlur)
  pointerQuery = null
  motionQuery = null
  layerRef.value = null
  dotRef.value = null
  ringRef.value = null
})
</script>

<template>
  <div
    ref="layerRef"
    class="jelly-cursor"
    :class="{ 'is-enabled': enabled, 'is-visible': visible }"
    aria-hidden="true"
  >
    <span ref="ringRef" class="jelly-cursor__ring"></span>
    <span ref="dotRef" class="jelly-cursor__dot"></span>
  </div>
</template>
