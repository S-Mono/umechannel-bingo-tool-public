<script setup lang="ts">
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import { nextTick, onMounted, onUnmounted, ref } from 'vue';

type EffectType = 'NORMAL_BINGO' | 'SPECIAL_1' | 'SPECIAL_25';

interface EffectPayload {
  effectType: EffectType;
  videoPath: string;
}

interface EffectHighlightPayload {
  active: boolean;
}

const videoElement = ref<HTMLVideoElement | null>(null);
const videoSource = ref('');
const isVisible = ref(false);
const isHighlightVisible = ref(false);
const currentEffectType = ref<EffectType | null>(null);
let playbackRequestId = 0;

let unlistenEffect: (() => void) | null = null;
let unlistenHighlight: (() => void) | null = null;
let unlistenStopPlayback: (() => void) | null = null;

const waitForVideoReady = (element: HTMLVideoElement) => new Promise<void>((resolve, reject) => {
  const cleanup = () => {
    element.removeEventListener('canplay', handleReady);
    element.removeEventListener('error', handleError);
  };

  const handleReady = () => {
    cleanup();
    resolve();
  };

  const handleError = () => {
    cleanup();
    reject(new Error('video load error'));
  };

  element.addEventListener('canplay', handleReady, { once: true });
  element.addEventListener('error', handleError, { once: true });
});

const hideWindow = async () => {
  playbackRequestId += 1;
  const stoppedEffectType = currentEffectType.value;
  currentEffectType.value = null;
  isVisible.value = false;
  videoSource.value = '';
  const element = videoElement.value;
  if (element) {
    element.pause();
    element.removeAttribute('src');
    element.load();
  }
  try {
    await invoke('hide_effect_window');
  } catch (error) {
    console.error('Failed to hide effect window:', error);
  }

  await emit('effect-playback-stopped', { effectType: stoppedEffectType });
};

const playVideo = async (payload: EffectPayload) => {
  const requestId = ++playbackRequestId;

  if (!payload.videoPath) {
    await hideWindow();
    return;
  }

  isVisible.value = false;
  currentEffectType.value = payload.effectType;
  videoSource.value = convertFileSrc(payload.videoPath);

  await nextTick();

  const element = videoElement.value;
  if (!element) {
    await hideWindow();
    return;
  }

  element.currentTime = 0;
  element.load();

  try {
    await waitForVideoReady(element);
    if (requestId !== playbackRequestId) {
      return;
    }

    isVisible.value = true;
    await emit('effect-playback-started', { effectType: payload.effectType });
    await element.play();
  } catch (error) {
    console.error('Failed to play effect video:', error);
    await hideWindow();
  }
};

onMounted(async () => {
  unlistenEffect = await listen<EffectPayload>('play-bingo-effect', async (event) => {
    await playVideo(event.payload);
  });

  unlistenHighlight = await listen<EffectHighlightPayload>('effect-monitor-highlight', (event) => {
    isHighlightVisible.value = event.payload.active;
  });

  unlistenStopPlayback = await listen('stop-effect-playback', async () => {
    await hideWindow();
  });
});

onUnmounted(() => {
  if (unlistenEffect) {
    unlistenEffect();
    unlistenEffect = null;
  }

  if (unlistenHighlight) {
    unlistenHighlight();
    unlistenHighlight = null;
  }

  if (unlistenStopPlayback) {
    unlistenStopPlayback();
    unlistenStopPlayback = null;
  }
});
</script>

<template>
  <div class="effect-root">
    <div class="effect-shell">
      <video
        ref="videoElement"
        class="effect-video"
        :class="{ visible: isVisible }"
        preload="auto"
        playsinline
        @ended="hideWindow"
        @error="hideWindow"
      >
        <source :src="videoSource" type="video/mp4" />
      </video>
      <div class="monitor-highlight" :class="{ visible: isHighlightVisible }"></div>
    </div>
  </div>
</template>

<style scoped>
.effect-root {
  width: 100vw;
  height: 100vh;
  overflow: hidden;
  background: transparent;
  pointer-events: none;
}

.effect-shell {
  position: relative;
  width: 100%;
  height: 100%;
  background: transparent;
  pointer-events: none;
}

.effect-video {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  background: transparent;
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.12s ease;
}

.effect-video.visible {
  opacity: 1;
}

.monitor-highlight {
  position: absolute;
  inset: 0;
  opacity: 0;
  background:
    radial-gradient(circle at center, rgba(52, 152, 219, 0.24) 0%, rgba(52, 152, 219, 0.12) 38%, rgba(52, 152, 219, 0.04) 68%, rgba(52, 152, 219, 0) 100%),
    rgba(52, 152, 219, 0.08);
  box-shadow: inset 0 0 140px rgba(52, 152, 219, 0.35);
  transition: opacity 0.16s ease;
  pointer-events: none;
}

.monitor-highlight.visible {
  opacity: 1;
}
</style>