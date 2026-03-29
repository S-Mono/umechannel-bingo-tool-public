<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { listen } from '@tauri-apps/api/event';

const BG_IMAGE_PATH = '/assets/background.png';
const HIT_MARK_IMAGE_PATH = '/assets/hit_mark.png';

// 初期値に hit_scale を追加
const gridPos = ref({ x: 22, y: 103, w: 237, h: 239, hit_scale: 100 });
const hitNumbers = ref<number[]>([]);

onMounted(async () => {
    await listen<any>('grid-update', (event) => {
        gridPos.value = event.payload;
    }
    );
    await listen<any>('bingo-hit', (event) => {
        if (!hitNumbers.value.includes(event.payload.number)) {
            hitNumbers.value.push(event.payload.number);
        }
    });
    // 追加: リセットイベントの受診
    await listen('bingo-reset', () => { hitNumbers.value = []; });
});
</script>

<template>
    <div class="bingo-view-container" data-tauri-drag-region>
        <img :src="BG_IMAGE_PATH" class="card-bg-img" alt="Bingo Card Background" />

        <div class="grid-layer" :style="{
            left: gridPos.x + 'px', top: gridPos.y + 'px',
            width: gridPos.w + 'px', height: gridPos.h + 'px',
            pointerEvents: 'none'
        }">
            <div v-for="n in 25" :key="n" class="cell">
                <span class="cell-num">{{ n }}</span>
                <transition name="pop">
                    <img v-if="hitNumbers.includes(n)" :src="HIT_MARK_IMAGE_PATH" class="hit-mark-img" :style="{
                        /* 100%の時に元の130%のサイズになるように係数を設定 */
                        width: (gridPos.hit_scale * 1.4) + '%',
                        height: (gridPos.hit_scale * 1.4) + '%',
                    }" />
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
    /* 重要: 枠なしウィンドウで確実にドラッグさせるための設定 */
    cursor: move;
    -webkit-app-region: drag;
}

.card-bg-img {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    pointer-events: none;
}

.grid-layer {
    position: absolute;
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    grid-template-rows: repeat(5, 1fr);
    gap: 1px;
    /* 子要素（数字やスタンプ）がドラッグ操作を邪魔しないようにする */
    pointer-events: none;
}

.cell {
    /* 修正：Gridレイアウトを採用し、子要素（スタンプ）を中央に強制配置 */
    display: grid;
    place-items: center;
    position: relative;
    overflow: hidden;
}

.cell-num {
    font-size: 8px;
    position: absolute;
    top: 2px;
    left: 2px;
    color: #fff;
    opacity: 0.3;
}

/* hit-mark-img の固定の width/height を削除、または基準値にする */
.hit-mark-img {
    /* 重要：position: absolute を削除しました。
           これにより、座標の競合がなくなり、中央からズレなくなります。
        */
    object-fit: contain;
    z-index: 10;
    pointer-events: none;
    /* アニメーションの基準点を中央に固定 */
    transform-origin: center center;
}

/* 修正：アニメーションの各ステップに中央寄せ (translate) を含める */
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