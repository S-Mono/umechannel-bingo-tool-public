<script setup lang="ts">
import { convertFileSrc } from '@tauri-apps/api/core';
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
const idleBackgroundUrl = '/assets/effect_idle_background.svg';
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

const stopPlayback = async () => {
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
  await emit('effect-playback-stopped', { effectType: stoppedEffectType });
};

const playVideo = async (payload: EffectPayload) => {
  const requestId = ++playbackRequestId;

  if (!payload.videoPath) {
    await stopPlayback();
    return;
  }

  isVisible.value = false;
  currentEffectType.value = payload.effectType;
  videoSource.value = convertFileSrc(payload.videoPath);

  await nextTick();

  const element = videoElement.value;
  if (!element) {
    await stopPlayback();
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
    await stopPlayback();
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
    await stopPlayback();
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
      <div class="idle-background" :class="{ hidden: isVisible }"></div>
      <img class="idle-background-image" :class="{ hidden: isVisible }" :src="idleBackgroundUrl" alt="" />
      <video
        ref="videoElement"
        class="effect-video"
        :class="{ visible: isVisible }"
        preload="auto"
        playsinline
        @ended="stopPlayback"
        @error="stopPlayback"
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
  background: #00ff00;
}

.effect-shell {
  position: relative;
  width: 100%;
  height: 100%;
  background: #00ff00;
}

.idle-background {
  position: absolute;
  inset: 0;
  background: #00ff00;
  opacity: 1;
  transition: opacity 0.12s ease;
}

.idle-background-image {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  opacity: 1;
  transition: opacity 0.12s ease;
}

.idle-background.hidden,
.idle-background-image.hidden {
  opacity: 0;
}

.effect-video {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: contain;
  background: #00ff00;
  opacity: 0;
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
}

.monitor-highlight.visible {
  opacity: 1;
}
</style>