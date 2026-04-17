<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import ControlPanel from './components/ControlPanel.vue';
import BingoCard from './components/BingoCard.vue';
import EffectOverlay from './components/EffectOverlay.vue';

const windowLabel = ref('');

onMounted(() => {
  windowLabel.value = getCurrentWindow().label;
});
</script>

<template>
  <ControlPanel v-if="windowLabel === 'main'" />
  <BingoCard v-else-if="windowLabel === 'display'" />
  <EffectOverlay v-else-if="windowLabel === 'effect'" />
</template>

<style>
/* 全体共通：スクロールバーを消し、背景を透過可能に */
html,
body,
#app {
  margin: 0;
  padding: 0;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  background-color: transparent;
  font-family: sans-serif;
}
</style>