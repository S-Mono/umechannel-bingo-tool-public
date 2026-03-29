<script setup lang="ts">
import { ref, watch, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';
import { getAllWindows } from '@tauri-apps/api/window';

// px単位での位置・サイズ
const grid = ref({ x: 22, y: 103, w: 237, h: 239 });

onMounted(async () => {
    try {
        const saved = await invoke<any>('load_settings');
        // Rustから取得したオブジェクトに window_x 等が含まれるため、分割して必要な値だけをReactiveにする
        grid.value = { x: saved.x, y: saved.y, w: saved.w, h: saved.h };
    } catch (e) { console.error(e); }
});

const saveToLocal = async () => {
    let window_x = null;
    let window_y = null;
    try {
        const windows = await getAllWindows();
        const displayWin = windows.find(w => w.label === 'display');
        if (displayWin) {
            const pos = await displayWin.outerPosition();
            window_x = pos.x;
            window_y = pos.y;
        }
    } catch (e) {
        console.error("Failed to get window position", e);
    }

    await invoke('save_settings', { config: { ...grid.value, window_x, window_y } });
    alert("設定を保存しました（bingo_config.json）");
};

watch(grid, (newVal) => {
    emit('grid-update', { ...newVal });
}, { deep: true });
</script>

<style scoped>
.panel {
    background-color: #2f2f2f;
    /* 操作パネルらしい暗めの背景色 */
    color: #fff;
    height: 100vh;
    padding: 20px;
    box-sizing: border-box;
}

.sliders label {
    display: block;
    margin-bottom: 10px;
}

.save-btn {
    margin-top: 20px;
    padding: 10px 20px;
    cursor: pointer;
}
</style>

<template>
    <div class="panel">
        <h3>📏 グリッド微調整 (px)</h3>
        <div class="sliders">
            <label>X位置: {{ grid.x }}px <input type="range" min="0" max="282" v-model.number="grid.x" /></label>
            <label>Y位置: {{ grid.y }}px <input type="range" min="0" max="368" v-model.number="grid.y" /></label>
            <label>横幅: {{ grid.w }}px <input type="range" min="0" max="282" v-model.number="grid.w" /></label>
            <label>縦幅: {{ grid.h }}px <input type="range" min="0" max="368" v-model.number="grid.h" /></label>
        </div>
        <button @click="saveToLocal" class="save-btn">設定を保存</button>
    </div>
</template>