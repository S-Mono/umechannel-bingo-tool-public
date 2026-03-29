<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { listen } from '@tauri-apps/api/event';

/**
 * 【注意】publicフォルダ内のアセットは直接パス文字列で指定します。
 * 拡張子が .jpg か .png か、実際のファイル名と完全に一致させてください。
 */
const BG_PATH = '/assets/background.jpg';
const HIT_MARK_PATH = '/assets/hit-mark.png';

// 状態管理
const gridPos = ref({ x: 22, y: 103, w: 237, h: 239 });
const hitNumbers = ref<number[]>([]);

onMounted(async () => {
    console.log("Display window: Initialized.");

    // 操作パネルからの位置更新イベントを購読
    await listen<any>('grid-update', (event) => {
        gridPos.value = event.payload;
    });

    // ビンゴヒット（当選）イベントを購読
    await listen<any>('bingo-hit', (event) => {
        if (!hitNumbers.value.includes(event.payload.number)) {
            hitNumbers.value.push(event.payload.number);
        }
    });
});
</script>

<template>
    <div class="bingo-view-container" data-tauri-drag-region>
        <img :src="BG_PATH" class="card-bg-img" alt="Bingo Card Background" />

        <div class="grid-layer" :style="{
            left: gridPos.x + 'px',
            top: gridPos.y + 'px',
            width: gridPos.w + 'px',
            height: gridPos.h + 'px'
        }">
            <div v-for="n in 25" :key="n" class="cell">
                <span class="cell-num">{{ n }}</span>
                <transition name="pop">
                    <img v-if="hitNumbers.includes(n)" :src="HIT_MARK_PATH" class="hit-mark-img" />
                </transition>
            </div>
        </div>
    </div>
</template>

<style scoped>
.bingo-view-container {
    width: 100%;
    height: 100%;
    position: relative;
    overflow: hidden;
    background-color: transparent;
    cursor: move;
    /* 枠なしウィンドウを移動可能にするためのヒント */
}

.card-bg-img {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    pointer-events: none;
    /* ドラッグ操作の邪魔をしない */
}

.grid-layer {
    position: absolute;
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    grid-template-rows: repeat(5, 1fr);
    gap: 1px;
    cursor: default;
}

.cell {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    /* border: 1px solid rgba(255, 255, 255, 0.1); // デバッグ用 */
}

.cell-num {
    font-size: 8px;
    position: absolute;
    top: 2px;
    left: 2px;
    color: #fff;
    opacity: 0.3;
}

.hit-mark-img {
    position: absolute;
    width: 95%;
    height: 95%;
    object-fit: contain;
    pointer-events: none;
    z-index: 10;
}

.pop-enter-active {
    animation: pop-in 0.3s cubic-bezier(0.175, 0.885, 0.32, 1.275);
}

@keyframes pop-in {
    0% {
        transform: scale(0);
        opacity: 0;
    }

    100% {
        transform: scale(1);
        opacity: 1;
    }
}
</style>