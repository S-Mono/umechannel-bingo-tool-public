<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { listen } from '@tauri-apps/api/event';

// アセットパス定義
const BG_IMAGE_PATH = '/assets/background.png';
const HIT_MARK_IMAGE_PATH = '/assets/hit_mark.png';

// 音声ファイル定義 (public/assets/audio/ 内に配置してください)
const SE_SPIN_PATH = '/assets/audio/spin_loop.mp3';
const SE_WIN_PATH = '/assets/audio/win_confirm.mp3';

// --- 状態管理 ---
// グリッド配置情報
const gridPos = ref({ x: 22, y: 103, w: 237, h: 239, hit_scale: 100 });
// 当選マスリスト
const hitNumbers = ref<number[]>([]);

// ルーレット状態
const isSpinning = ref(false); // 抽選中フラグ
const rouletteNumber = ref<number | null>(null); // ルーレットに表示する数字
let spinAudio: HTMLAudioElement | null = null; // ループ音のオーディオオブジェクト

// --- 音響効果 & 音声合成 ---
const synth = window.speechSynthesis;

const speakNumber = (num: number) => {
    if (!('speechSynthesis' in window)) return;
    synth.cancel(); // 連続再生時に前の音声をキャンセル
    const utterance = new SpeechSynthesisUtterance();
    utterance.text = num.toString();
    utterance.lang = 'ja-JP';
    utterance.rate = 1.2;
    synth.speak(utterance);
};

const playWinSound = () => {
    // 配信キャプチャ用に音量を調整
    const audio = new Audio(SE_WIN_PATH);
    audio.volume = 0.7; // 少し音量を下げる
    audio.play().catch(e => console.error("Audio play failed:", e));
};

// --- ルーレットアニメーションロジック ---
let rouletteIntervalId: number | null = null;

const startRouletteAnimation = (finalNumber: number) => {
    if (isSpinning.value) return; // 二重再生防止
    isSpinning.value = true;

    // ループ音再生開始
    if (!spinAudio) {
        spinAudio = new Audio(SE_SPIN_PATH);
        spinAudio.loop = true;
        spinAudio.volume = 0.5; // 音量を調整
    }
    spinAudio.play().catch(e => console.error("Spin Audio failed:", e));

    // 高速で数字を切り替える (slot machine効果)
    rouletteIntervalId = setInterval(() => {
        rouletteNumber.value = Math.floor(Math.random() * 25) + 1; // 1-25の乱数
    }, 50); // 50ms間隔

    // 演出時間（1.5秒。OBS遅延を考慮）
    setTimeout(() => {
        stopRouletteAnimation(finalNumber);
    }, 1500);
};

const stopRouletteAnimation = (finalNumber: number) => {
    if (rouletteIntervalId) {
        clearInterval(rouletteIntervalId); // 高速切り替えを停止
        rouletteIntervalId = null;
    }

    // ループ音停止
    spinAudio?.pause();
    if (spinAudio) spinAudio.currentTime = 0;

    isSpinning.value = false;
    rouletteNumber.value = finalNumber; // 真の当選番号をセット

    // 当選時の演出
    playWinSound(); // 効果音
    speakNumber(finalNumber); // 読み上げ

    // 【重要】アニメーション完了後にグリッドへスタンプを反映
    if (!hitNumbers.value.includes(finalNumber)) {
        hitNumbers.value.push(finalNumber);
    }
};

// --- タウリイベント受診 (IPC LISTENERS) ---
onMounted(async () => {
    console.log("Display window: Initialized with Fixed Roulette Display.");

    // 操作パネルからの位置・縮尺更新
    await listen<any>('grid-update', (event) => { gridPos.value = event.payload; });

    // ビンゴ当選イベント
    await listen<{ number: number }>('bingo-hit', (event) => {
        const finalNum = event.payload.number;
        if (finalNum > 0 && finalNum <= 25) {
            // 即座にスタンプを押さず、ルーレットを起動
            startRouletteAnimation(finalNum);
        }
    });

    // リセットイベント
    await listen('bingo-reset', () => {
        hitNumbers.value = [];
        rouletteNumber.value = null; // ルーレット表示もリセット
    });
});
</script>

<template>
    <div class="bingo-view-container" data-tauri-drag-region>
        <img :src="BG_IMAGE_PATH" class="card-bg-img" alt="Bingo Card Background" />

        <div class="lottery-display-area" :style="{
            backgroundColor: 'rgba(0, 0, 0, 0.7)', /* 半透明の黒背景で数字を際立たせる */
            top: '5%',
            left: '50%', transform: 'translateX(-50%)',
            width: '80px', height: '45px',
            borderRadius: '8px', /* 少し角を丸くしてなじませる */
            boxShadow: '0 4px 6px rgba(0,0,0,0.3)' /* 影をつけて浮き立たせる */
        }">
            <transition name="lottery-fade">
                <span v-if="rouletteNumber !== null" class="roulette-number" :class="{ 'is-confirming': isSpinning }">
                    {{ rouletteNumber }}
                </span>
            </transition>
        </div>

        <div class="grid-layer" :style="{
            left: gridPos.x + 'px', top: gridPos.y + 'px',
            width: gridPos.w + 'px', height: gridPos.h + 'px',
            pointerEvents: 'none'
        }">
            <div v-for="n in 25" :key="n" class="cell">
                <span class="cell-num">{{ n }}</span>
                <transition name="pop">
                    <img v-if="hitNumbers.includes(n)" :src="HIT_MARK_IMAGE_PATH" class="hit-mark-img" :style="{
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

/* --- 【新規】ルーレット表示スタイル --- */
.lottery-display-area {
    position: absolute;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
}

.roulette-number {
    font-size: 3rem;
    font-weight: bold;
    color: #f1c40f;
    transform: translateY(-4px);
    /* 念のため余計な余白をゼロに */
    /* 金色 */
    /* 白背景だと金色が見づらいので、黒い影を強くして視認性を確保 */
    text-shadow: 2px 2px 0 #000, -1px -1px 0 #000, 1px -1px 0 #000, -1px 1px 0 #000, 1px 1px 0 #000;
}

/* 抽選中は少し半透明にして slot-machine 感を出す演出 */
.is-confirming {
    opacity: 0.6;
}

.lottery-fade-enter-active,
.lottery-fade-leave-active {
    transition: opacity 0.3s, transform 0.3s;
}

.lottery-fade-enter-from,
.lottery-fade-leave-to {
    opacity: 0;
    transform: translateY(-10px);
}

/* --- グリッド配置スタイル（変更なし） --- */
.grid-layer {
    position: absolute;
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    grid-template-rows: repeat(5, 1fr);
    gap: 1px;
    pointer-events: none;
}

.cell {
    position: relative;
    display: grid;
    place-items: center;
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

.hit-mark-img {
    object-fit: contain;
    z-index: 10;
    pointer-events: none;
    transform-origin: center center;
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