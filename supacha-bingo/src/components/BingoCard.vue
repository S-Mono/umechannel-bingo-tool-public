<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { listen } from '@tauri-apps/api/event';
import confetti from 'canvas-confetti';

// アセットパス定義
const BG_IMAGE_PATH = '/assets/background.png';
const HIT_MARK_IMAGE_PATH = '/assets/hit_mark.png';

// 音声ファイル定義 (public/assets/audio/ 内に配置してください)
const SE_SPIN_PATH = '/assets/audio/spin_loop.mp3';
const SE_WIN_PATH = '/assets/audio/win_confirm.mp3';

// --- 状態管理 ---
// グリッド配置情報
const gridPos = ref({
    x: 22, y: 109, w: 237, h: 239, hit_scale: 100,
    se_enabled: true, se_volume: 20,
    tts_enabled: true, tts_volume: 40, tts_repeat_count: 1
});
// 当選マスリスト
const hitNumbers = ref<number[]>([]);

// ルーレット状態
const isSpinning = ref(false); // 抽選中フラグ
const rouletteNumber = ref<number | null>(null); // ルーレットに表示する数字
let spinAudio: HTMLAudioElement | null = null; // ループ音のオーディオオブジェクト

// 【新規】紙吹雪用のcanvas要素のref
const confettiCanvas = ref<HTMLCanvasElement | null>(null);
let myConfetti: confetti.CreateTypes | null = null; // インスタンスを保持

// 読み上げ関数の修正（回数指定に対応）
const speakNumber = (num: number) => {
    if (!gridPos.value.tts_enabled) return;
    window.speechSynthesis.cancel();

    for (let i = 0; i < gridPos.value.tts_repeat_count; i++) {
        const utterance = new SpeechSynthesisUtterance(num.toString());
        utterance.lang = 'ja-JP';
        // 0-100の値を0.0-1.0に変換
        const gainFactor = 1.5;
        utterance.volume = Math.min(1.0, (gridPos.value.tts_volume / 100) * gainFactor);
        window.speechSynthesis.speak(utterance);
    }
};

const playWinSound = () => {
    // 【修正】有効チェック
    if (!gridPos.value.se_enabled) return;

    const audio = new Audio(SE_WIN_PATH);
    const gainFactor = 0.8;
    // 【修正】設定された音量を反映 (0-100 -> 0.0-1.0)
    audio.volume = Math.min(1.0, (gridPos.value.se_volume / 100) * gainFactor);
    audio.play().catch(e => console.error("Audio play failed:", e));
};

// --- 【新規】紙吹雪を発射するロジック ---
const fireConfetti = () => {
    if (!confettiCanvas.value) return;

    // 初回のみインスタンス化（パフォーマンス対策）
    if (!myConfetti) {
        myConfetti = confetti.create(confettiCanvas.value, {
            resize: true, // ウィンドウサイズ変更に追従
            useWorker: true // パフォーマンス向上のためWorkerを使用
        });
    }

    // 発射設定 (ツールの色に合わせる: 金, 赤, 白)
    const colors = ['#f1c40f', '#e74c3c', '#ffffff'];

    // 【提案】抽選エリア (top: 30px, left: 50%) を中心に、少し下向きに広がるように発射
    myConfetti({
        particleCount: 200, // 粒子数
        spread: 100, // 広がり（少し広めに）
        origin: { y: -0.1, x: 0.5 }, // ウィンドウ上部中央 (抽選エリア付近)
        colors: colors,
        gravity: 0.75, // 重力（少し軽めに長く降らせる）
        drift: 0, // 横方向のドリフト
        ticks: 350, // 粒子が消えるまでの時間（少し長めに）
        shapes: ['square', 'circle'], // 形
        scalar: 0.9 // 粒子の大きさ
    });
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
    }

    // 【修正】再生前に設定を適用
    if (gridPos.value.se_enabled) {
        const gainFactor = 0.5;
        spinAudio.volume = Math.min(1.0, (gridPos.value.se_volume / 100) * gainFactor);
        spinAudio.play().catch(e => console.error("Spin Audio failed:", e));
    }

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

    // 【新規】当選確定時に紙吹雪を発射
    fireConfetti();

    // 【重要】アニメーション完了後にグリッドへスタンプを反映
    if (!hitNumbers.value.includes(finalNumber)) {
        hitNumbers.value.push(finalNumber);
    }
};

// 【追加】枠線を表示するかどうかの状態
const showBorder = ref(false);

// --- タウリイベント受診 (IPC LISTENERS) ---
onMounted(async () => {
    console.log("Display window: Initialized with Fixed Roulette Display.");

    // 操作パネルからの位置・縮尺更新
    await listen<any>('grid-update', (event) => {
        gridPos.value = event.payload;
    });

    // 【追加】編集モードの状態更新をリッスン
    await listen<boolean>('edit-mode-update', (event) => {
        showBorder.value = event.payload;
    });

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

        <canvas ref="confettiCanvas" class="confetti-canvas"></canvas>

        <div class="lottery-display-area" :style="{
            backgroundColor: 'rgba(0, 0, 0, 0.7)', /* 半透明の黒背景で数字を際立たせる */
            top: '5%',
            left: '50%', transform: 'translateX(-50%)',
            width: '80px', height: '45px',
            borderRadius: '8px', /* 少し角を丸くしてなじませる */
            boxShadow: '0 4px 6px rgba(0,0,0,0.3)' /* 影をつけて浮き立たせる */
        }">
            <transition name="lottery-fade">
                <span class="roulette-number" :class="{ 'is-confirming': isSpinning }">
                    {{ rouletteNumber == null ? '？' : rouletteNumber }}
                </span>
            </transition>
        </div>

        <div class="grid-layer" :style="{
            left: gridPos.x + 'px', top: gridPos.y + 'px',
            width: gridPos.w + 'px', height: gridPos.h + 'px',
            pointerEvents: 'none'
        }">
            <div v-for="n in 25" :key="n" class="cell" :class="{ 'show-border': showBorder }">
                <span class="cell-num">{{ n }}</span>
                <transition name="pop">
                    <img v-if="hitNumbers.includes(n)" :src="HIT_MARK_IMAGE_PATH" class="hit-mark-img" :style="{
                        width: (gridPos.hit_scale * 1.4) + '%',
                        height: (gridPos.hit_scale * 1) + '%',
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

/* 3. canvas のスタイル。ウィンドウ全体に広げ、マウスイベントを透過させる */
.confetti-canvas {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    z-index: 5;
    /* スタンプ (10) より下、背景より上 */
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
    /* デフォルトでは枠線を透明にしておくことで、ガタつきを防ぐ */
    border: 1px solid transparent;
    box-sizing: border-box;
    /* 枠線が表示されてもサイズが変わらないように設定 */
}

/* 【追加】編集モード中の枠線スタイル */
.cell.show-border {
    border: 1px solid rgba(255, 0, 0, 0.8);
    /* 半透明の赤で表示 */
}

.cell-num {
    font-size: 10px;
    position: absolute;
    top: 0px;
    left: 2px;
    color: #fff;
    /* 通常時は消しておく */
    opacity: 0;
    /* 滑らかに表示 */
    transition: opacity 0.2s;
}

/* 修正ポイント3: 編集中の数字を表示 */
.cell.show-border .cell-num {
    color: rgba(255, 0, 0, 0.8);
    opacity: 1;
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