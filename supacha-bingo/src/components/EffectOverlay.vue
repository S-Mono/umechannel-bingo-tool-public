<script setup lang="ts">
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { nextTick, onMounted, onUnmounted, ref } from 'vue';

type EffectType = 'NORMAL_BINGO' | 'SPECIAL_1' | 'SPECIAL_25';

interface EffectPayload {
  effectType: EffectType;
  videoPath: string;
}

const videoElement = ref<HTMLVideoElement | null>(null);
const videoSource = ref('');
const isVisible = ref(false);

let unlistenEffect: (() => void) | null = null;

const hideWindow = async () => {
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
};

const playVideo = async (payload: EffectPayload) => {
  if (!payload.videoPath) {
    await hideWindow();
    return;
  }

  videoSource.value = convertFileSrc(payload.videoPath);
  isVisible.value = true;

  await nextTick();

  const element = videoElement.value;
  if (!element) {
    await hideWindow();
    return;
  }

  element.currentTime = 0;
  element.load();

  try {
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
});

onUnmounted(() => {
  if (unlistenEffect) {
    unlistenEffect();
    unlistenEffect = null;
  }
});
</script>

<template>
  <div class="effect-root" :class="{ visible: isVisible }">
    <div v-if="isVisible" class="effect-shell">
      <video
        ref="videoElement"
        class="effect-video"
        preload="auto"
        playsinline
        @ended="hideWindow"
        @error="hideWindow"
      >
        <source :src="videoSource" type="video/mp4" />
      </video>
    </div>
  </div>
</template>

<style scoped>
.effect-root {
  width: 100vw;
  height: 100vh;
  overflow: hidden;
  background: transparent;
  opacity: 0;
}

.effect-root.visible {
  opacity: 1;
}

.effect-shell {
  position: relative;
  width: 100%;
  height: 100%;
  background: rgba(0, 0, 0, 0.01);
}

.effect-video {
  width: 100%;
  height: 100%;
  object-fit: cover;
  background: transparent;
}
</style>