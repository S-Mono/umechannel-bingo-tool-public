<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { getAllWebviewWindows } from '@tauri-apps/api/webviewWindow';
import confetti from 'canvas-confetti';

/** --- アセット・音声定義 --- */
const BG_IMAGE_PATH = '/assets/background.png';
const HIT_MARK_IMAGE_PATH = '/assets/hit_mark.png';
const SE_SPIN_PATH = '/assets/audio/spin_loop.mp3';
const SE_WIN_PATH = '/assets/audio/win_confirm.mp3';

/** --- 状態管理 --- */
const gridPos = ref({
    x: 22, y: 109, w: 237, h: 239, hit_scale: 100,
    se_enabled: true, se_volume: 20, tts_enabled: true, tts_volume: 40, tts_repeat_count: 1
});
const hitNumbers = ref<number[]>([]);
const isSpinning = ref(false);
const rouletteNumber = ref<number | null>(null);
const showBorder = ref(false);
let spinAudio: HTMLAudioElement | null = null;

/** --- ウィンドウ操作ロジック --- */
const appWindow = getCurrentWindow();
// ウィンドウ移動を開始する関数
const startWindowDrag = async (e: MouseEvent) => {
    // 左クリック（button: 0）の時のみドラッグを開始
    if (e.button === 0) {
        try {
            await appWindow.startDragging();
        } catch (err) {
            console.error("Failed to start dragging:", err);
        }
    }
};

/** --- コンテキストメニュー管理 --- */
const showMenu = ref(false);
const menuPos = ref({ x: 0, y: 0 });
const handleContextMenu = (e: MouseEvent) => {
    e.preventDefault();

    // メニューの物理サイズ（CSSで160pxに設定している場合）
    const menuWidth = 160;
    const menuHeight = 110; // 項目数に応じた高さ

    // 現在のウィンドウサイズを取得
    const winWidth = window.innerWidth;
    const winHeight = window.innerHeight;

    let x = e.clientX;
    let y = e.clientY;

    // 【衝突判定】右端・下端の境界チェック
    if (x + menuWidth > winWidth) x = winWidth - menuWidth - 8;
    if (y + menuHeight > winHeight) y = winHeight - menuHeight - 8;

    menuPos.value = { x, y };
    showMenu.value = true;
};

// 設定画面（main）を前面に出す（修正版）
const openSettings = async () => {
    closeMenu();
    try {
        // 全てのWebviewWindowインスタンスを取得（これが最も確実です）
        const windows = await getAllWebviewWindows();
        const mainWindow = windows.find(w => w.label === 'main');

        if (mainWindow) {
            // Tauri v2 ではこれらのメソッドは全て Promise を返します
            await mainWindow.unminimize(); // 最小化解除
            await mainWindow.show();       // 表示
            await mainWindow.setFocus();   // フォーカス（最前面）
        } else {
            console.error("Main window (label: 'main') not found.");
        }
    } catch (err) {
        // ここでエラーが出る場合は、capabilities/default.json の権限不足です
        console.error("Window operation failed:", err);
    }
};
const closeMenu = () => { showMenu.value = false; };

const exitGame = async () => {
    closeMenu();
    await invoke('exit_app'); // Rustコマンド
};

// 【重要】グローバルなクリックとキー入力を監視する関数
const handleGlobalEvents = (e: Event) => {
    if (!showMenu.value) return;

    // 左クリックされた、または Escキーが押されたら閉じる
    if (e instanceof KeyboardEvent && e.key === 'Escape') {
        closeMenu();
        return;
    }

    if (e instanceof MouseEvent) {
        // e.button === 0 (左クリック) の場合のみ閉じる
        // これにより、右クリック（ボタン2）でメニューを開く動作と干渉しなくなります
        if (e.button === 0) {
            closeMenu();
        }
    }
};

// メニューの表示状態を監視してリスナーを登録/解除
watch(showMenu, (newVal) => {
    if (newVal) {
        window.addEventListener('mousedown', handleGlobalEvents);
        window.addEventListener('keydown', handleGlobalEvents);
    } else {
        window.removeEventListener('mousedown', handleGlobalEvents);
        window.removeEventListener('keydown', handleGlobalEvents);
    }
});

// コンポーネントが消える時にリスナーを確実に掃除（メモリリーク防止）
onUnmounted(() => {
    window.removeEventListener('click', handleGlobalEvents);
    window.removeEventListener('keydown', handleGlobalEvents);
});

/** --- 演出ロジック --- */
const speakNumber = (num: number) => {
    if (!gridPos.value.tts_enabled) return;
    window.speechSynthesis.cancel();
    for (let i = 0; i < gridPos.value.tts_repeat_count; i++) {
        const u = new SpeechSynthesisUtterance(num.toString());
        u.lang = 'ja-JP';
        u.volume = gridPos.value.tts_volume / 100;
        window.speechSynthesis.speak(u);
    }
};

const playWinSound = () => {
    if (!gridPos.value.se_enabled) return;
    const audio = new Audio(SE_WIN_PATH);
    audio.volume = gridPos.value.se_volume / 100;
    audio.play().catch(console.error);
};

const startRoulette = (final: number) => {
    if (isSpinning.value) return;
    isSpinning.value = true;
    if (gridPos.value.se_enabled) {
        if (!spinAudio) {
            spinAudio = new Audio(SE_SPIN_PATH);
            spinAudio.loop = true;
        }
        // 設定からSE音量（0-100）を取得
        const baseVolume = gridPos.value.se_volume / 100;
        // SPIN SE専用の減衰係数（例: 0.6倍にする）
        const SPIN_GAIN_COEFFICIENT = 0.3;
        // 音量を設定して再生（ループするのでcurrentTimeもリセット）
        spinAudio.currentTime = 0;
        spinAudio.volume = baseVolume * SPIN_GAIN_COEFFICIENT;
        spinAudio.play().catch(console.error);
    }
    const interval = setInterval(() => {
        rouletteNumber.value = Math.floor(Math.random() * 25) + 1;
    }, 60);
    setTimeout(() => {
        clearInterval(interval);
        finishRoulette(final);
    }, 1800);
};

const finishRoulette = (final: number) => {
    isSpinning.value = false;
    rouletteNumber.value = final;
    if (spinAudio) spinAudio.pause();
    if (!hitNumbers.value.includes(final)) hitNumbers.value.push(final);
    playWinSound();
    speakNumber(final);
    confetti({ particleCount: 150, spread: 70, origin: { y: 0.6 } });
    emit('bingo-animation-finished', { number: final });
};

/** --- ライフサイクル --- */
let unlistenUpdate: any, unlistenEdit: any, unlistenHit: any, unlistenReset: any, unlistenSync: any;

onMounted(async () => {
    unlistenUpdate = await listen<any>('grid-update', (e) => { gridPos.value = e.payload; });
    unlistenEdit = await listen<boolean>('edit-mode-update', (e) => { showBorder.value = e.payload; });
    unlistenHit = await listen<{ number: number }>('bingo-hit', (e) => { startRoulette(e.payload.number); });
    unlistenReset = await listen('bingo-reset', () => { hitNumbers.value = []; rouletteNumber.value = null; });
    unlistenSync = await listen<{ hits: number[] }>('bingo-sync-hits', (e) => {
        hitNumbers.value = e.payload.hits;
        rouletteNumber.value = null;
    });
});

onUnmounted(() => {
    [unlistenUpdate, unlistenEdit, unlistenHit, unlistenReset, unlistenSync].forEach(u => u && u());
    if (spinAudio) { spinAudio.pause(); spinAudio = null; }
});
</script>

<template>
    <div class="bingo-view-container" @mousedown="startWindowDrag" @contextmenu.prevent="handleContextMenu"
        @click="closeMenu">

        <img :src="BG_IMAGE_PATH" class="card-bg-img" />

        <div class="lottery-display-area">
            <span class="roulette-number">{{ rouletteNumber == null ? '？' : rouletteNumber }}</span>
        </div>

        <div class="grid-layer" :style="{
            left: gridPos.x + 'px', top: gridPos.y + 'px',
            width: gridPos.w + 'px', height: gridPos.h + 'px'
        }">
            <div v-for="n in 25" :key="n" class="cell" :class="{ 'editing': showBorder }">
                <span v-if="showBorder" class="guide-num">{{ n }}</span>
                <transition name="pop">
                    <img v-if="hitNumbers.includes(n)" :src="HIT_MARK_IMAGE_PATH" class="stamp" :style="{
                        width: (gridPos.hit_scale * 1.5) + '%',
                        height: (gridPos.hit_scale * 1.5) + '%'
                    }" />
                </transition>
            </div>
        </div>

        <Teleport to="body">
            <div v-if="showMenu" class="custom-context-menu" :style="{ top: menuPos.y + 'px', left: menuPos.x + 'px' }"
                @click.stop @mousedown.stop>
                <div class="menu-item" @click="openSettings">⚙️ 設定画面を開く</div>
                <div class="menu-divider"></div>
                <div class="menu-item exit" @click="exitGame">❌ ゲームを終了</div>
            </div>
        </Teleport>
    </div>
</template>

<style scoped>
.bingo-view-container {
    width: 100vw;
    height: 100vh;
    position: relative;
    overflow: hidden;
    background-color: rgba(0, 0, 0, 0);
    /* 【解決】手動ドラッグのため grab カーソルが有効になる */
    cursor: grab;
}

.bingo-view-container:active {
    cursor: grabbing;
}

.card-bg-img {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    pointer-events: none;
    /* ドラッグイベントをコンテナに透過させる */
    user-select: none;
    z-index: 1;
}

.lottery-display-area {
    position: absolute;
    top: 5%;
    left: 50%;
    transform: translateX(-50%);
    width: 80px;
    height: 45px;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 8px;
    z-index: 100;
    pointer-events: none;
}

.roulette-number {
    font-size: 2.5rem;
    line-height: 1;
    height: 100%;
    color: #f1c40f;
    font-weight: bold;
    text-shadow: 2px 2px 0 #000;
}

.grid-layer {
    position: absolute;
    pointer-events: none;
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    grid-template-rows: repeat(5, 1fr);
    z-index: 50;
}

.cell {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid transparent;
    overflow: hidden;
}

.cell.editing {
    border: 1px solid rgba(255, 0, 0, 0.5);
}

.guide-num {
    position: absolute;
    top: 2px;
    left: 2px;
    font-size: 10px;
    color: red;
}

.stamp {
    object-fit: contain;
    /* margin-top: -10px; */
    z-index: 10;
}

/* コンテキストメニューのスタイル */
.custom-context-menu {
    position: fixed;
    z-index: 10000;
    background: #1a2a3a;
    border: 1px solid #f39c12;
    border-radius: 4px;
    padding: 5px 0;
    width: 160px;
    box-shadow: 5px 5px 15px rgba(0, 0, 0, 0.5);
}

.menu-item {
    padding: 8px 15px;
    color: #eee;
    font-size: 0.85rem;
    cursor: pointer;
    transition: 0.2s;
}

.menu-item:hover {
    background: rgba(243, 156, 18, 0.2);
    color: #f39c12;
}

.menu-item.exit:hover {
    background: rgba(231, 76, 60, 0.2);
    color: #e74c3c;
}

.menu-divider {
    height: 1px;
    background: rgba(255, 255, 255, 0.1);
    margin: 5px 0;
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