<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { getAllWebviewWindows } from '@tauri-apps/api/webviewWindow';
import confetti from 'canvas-confetti';

/** --- アセット・音声定義 --- */
const BUNDLED_BG_IMAGE_PATH = '/assets/background.png';
const BUNDLED_HIT_MARK_IMAGE_PATH = '/assets/hit_mark.png';
const BUNDLED_SE_SPIN_PATH = '/assets/audio/spin_loop.mp3';
const BUNDLED_SE_WIN_PATH = '/assets/audio/win_confirm.mp3';

interface RuntimeAssetPaths {
    backgroundImagePath: string;
    hitMarkImagePath: string;
    spinSePath: string;
    winSePath: string;
}

/** --- 状態管理 --- */
const gridPos = ref({
    x: 22, y: 109, w: 237, h: 239, hit_scale: 100,
    se_enabled: true, se_volume: 20, tts_enabled: true, tts_volume: 40, tts_repeat_count: 1
});
const hitNumbers = ref<number[]>([]);
const isSpinning = ref(false);
const rouletteNumber = ref<number | null>(null);
const showBorder = ref(false);
const isEffectPlaying = ref(false);
const backgroundImagePath = ref(BUNDLED_BG_IMAGE_PATH);
const hitMarkImagePath = ref(BUNDLED_HIT_MARK_IMAGE_PATH);
const spinSePath = ref(BUNDLED_SE_SPIN_PATH);
const winSePath = ref(BUNDLED_SE_WIN_PATH);
let spinAudio: HTMLAudioElement | null = null;
const targetNum = ref<number | null>(null);
let spinInterval: number | null = null; // setIntervalの管理用
let displayWindowMoveSaveTimer: number | null = null;
let unlistenDisplayWindowMoved: (() => void) | null = null;
let unlistenEffectPlaybackStarted: (() => void) | null = null;
let unlistenEffectPlaybackStopped: (() => void) | null = null;

/** --- ウィンドウ操作ロジック --- */
const appWindow = getCurrentWindow();

const toRuntimeAssetUrl = (path: string) => /^[a-zA-Z]:[\\/]/.test(path) ? convertFileSrc(path) : path;

const applyRuntimeAssetPaths = (paths: RuntimeAssetPaths) => {
    backgroundImagePath.value = toRuntimeAssetUrl(paths.backgroundImagePath);
    hitMarkImagePath.value = toRuntimeAssetUrl(paths.hitMarkImagePath);
    spinSePath.value = toRuntimeAssetUrl(paths.spinSePath);
    winSePath.value = toRuntimeAssetUrl(paths.winSePath);
};

const resetBackgroundImageToFallback = () => {
    if (backgroundImagePath.value !== BUNDLED_BG_IMAGE_PATH) {
        backgroundImagePath.value = BUNDLED_BG_IMAGE_PATH;
    }
};

const resetHitMarkImageToFallback = () => {
    if (hitMarkImagePath.value !== BUNDLED_HIT_MARK_IMAGE_PATH) {
        hitMarkImagePath.value = BUNDLED_HIT_MARK_IMAGE_PATH;
    }
};

const createAudio = (path: string, fallbackPath: string, loop = false) => {
    const audio = new Audio(path);
    audio.loop = loop;

    if (path !== fallbackPath) {
        audio.addEventListener('error', () => {
            audio.src = fallbackPath;
            audio.load();
        }, { once: true });
    }

    return audio;
};

const saveDisplayWindowPosition = async (x: number, y: number) => {
    try {
        await invoke('save_window_position', { label: 'display', x, y });
    } catch (error) {
        console.error('Display Window Position Save Error:', error);
    }
};

const scheduleDisplayWindowPositionSave = (x: number, y: number) => {
    if (displayWindowMoveSaveTimer !== null) {
        window.clearTimeout(displayWindowMoveSaveTimer);
    }

    displayWindowMoveSaveTimer = window.setTimeout(async () => {
        await saveDisplayWindowPosition(x, y);
        displayWindowMoveSaveTimer = null;
    }, 150);
};

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
    const menuHeight = 148; // 項目数に応じた高さ

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

const openEffectWindow = async () => {
    closeMenu();
    try {
        await invoke('show_effect_window');
    } catch (err) {
        console.error('Effect window operation failed:', err);
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

const cleanupGlobalMenuListeners = () => {
    window.removeEventListener('mousedown', handleGlobalEvents);
    window.removeEventListener('keydown', handleGlobalEvents);
};

const requestEffectPlaybackStop = async () => {
    try {
        await emit('stop-effect-playback', {});
    } catch (error) {
        console.error('Effect Playback Stop Error:', error);
    }
};

const handleEscEffectStop = async (event: KeyboardEvent) => {
    if (event.key !== 'Escape' || !isEffectPlaying.value) {
        return;
    }

    event.preventDefault();
    event.stopPropagation();
    await requestEffectPlaybackStop();
};

const registerEscEffectStopListeners = () => {
    window.addEventListener('keydown', handleEscEffectStop, true);
    document.addEventListener('keydown', handleEscEffectStop, true);
};

const unregisterEscEffectStopListeners = () => {
    window.removeEventListener('keydown', handleEscEffectStop, true);
    document.removeEventListener('keydown', handleEscEffectStop, true);
};

// コンポーネントが消える時にリスナーを確実に掃除（メモリリーク防止）
onUnmounted(() => {
    cleanupGlobalMenuListeners();
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
    const audio = createAudio(winSePath.value, BUNDLED_SE_WIN_PATH);
    audio.volume = gridPos.value.se_volume / 100;
    audio.play().catch(console.error);
};

const startSpinning = () => {
    if (isSpinning.value) return;
    isSpinning.value = true;

    // 音声再生（既存の spinAudio ロジックをそのまま利用）
    if (gridPos.value.se_enabled) {
        if (!spinAudio) {
            spinAudio = createAudio(spinSePath.value, BUNDLED_SE_SPIN_PATH, true);
        }
        const baseVolume = gridPos.value.se_volume / 100;
        const SPIN_GAIN_COEFFICIENT = 1;
        spinAudio.currentTime = 0;
        spinAudio.volume = baseVolume * SPIN_GAIN_COEFFICIENT;
        spinAudio.play().catch(console.error);
    }

    // 高速シャッフル開始
    spinInterval = window.setInterval(() => {
        rouletteNumber.value = Math.floor(Math.random() * 25) + 1;
    }, 60);
};

// 【追加】ストップが押された時の確定処理
const stopSpinning = () => {
    if (spinInterval) {
        clearInterval(spinInterval);
        spinInterval = null;
    }

    isSpinning.value = false;
    if (spinAudio) spinAudio.pause();

    // 予約されていた番号をセット
    const final = targetNum.value || 0;
    rouletteNumber.value = final;

    // 既存の確定演出
    if (!hitNumbers.value.includes(final)) hitNumbers.value.push(final);
    playWinSound();
    speakNumber(final);
    confetti({ particleCount: 150, spread: 70, origin: { y: 0.6 } });

    // 1秒後にControlPanel側へ「完了」を通知（保存処理などを走らせるため）
    setTimeout(() => {
        emit('bingo-animation-finished', { number: final });
    }, 1000);
};

/** --- ライフサイクル --- */
let unlistenUpdate: any, unlistenEdit: any, unlistenHit: any, unlistenReset: any, unlistenSync: any, unlistenStop: any;

onMounted(async () => {
    // 起動時に自分でも設定ファイルから設定値を読み込む
    try {
        const saved = await invoke<any>('load_settings');
        gridPos.value = { ...gridPos.value, ...saved };
        const runtimeAssets = await invoke<RuntimeAssetPaths>('get_runtime_asset_paths');
        applyRuntimeAssetPaths(runtimeAssets);
    } catch (e) { console.error(e); }

    unlistenDisplayWindowMoved = await appWindow.onMoved((event) => {
        scheduleDisplayWindowPositionSave(event.payload.x, event.payload.y);
    });

    // イベントリスナーの登録
    unlistenUpdate = await listen<any>('grid-update', (e) => { gridPos.value = e.payload; });
    unlistenEdit = await listen<boolean>('edit-mode-update', (e) => { showBorder.value = e.payload; });
    unlistenEffectPlaybackStarted = await listen('effect-playback-started', () => { isEffectPlaying.value = true; });
    unlistenEffectPlaybackStopped = await listen('effect-playback-stopped', () => { isEffectPlaying.value = false; });
    //unlistenHit = await listen<{ number: number }>('bingo-hit', (e) => { startRoulette(e.payload.number); });
    unlistenReset = await listen('bingo-reset', () => { hitNumbers.value = []; rouletteNumber.value = null; });
    unlistenSync = await listen<{ hits: number[] }>('bingo-sync-hits', (e) => {
        hitNumbers.value = e.payload.hits;
        rouletteNumber.value = null;
    });
    // 抽選開始の指示を受け取る
    unlistenHit = await listen<{ number: number }>('bingo-spin-start', () => {
        startSpinning();
    });
    // 停止指示を受け取る (停止時に当選番号を受け取る)
    unlistenStop = await listen<{ number: number }>('bingo-spin-stop', (e) => {
        targetNum.value = e.payload.number;
        stopSpinning();
    });

    registerEscEffectStopListeners();
});

onUnmounted(() => {
    [unlistenUpdate, unlistenEdit, unlistenHit, unlistenReset, unlistenSync, unlistenStop].forEach(u => u && u());
    if (unlistenEffectPlaybackStarted) {
        unlistenEffectPlaybackStarted();
        unlistenEffectPlaybackStarted = null;
    }
    if (unlistenEffectPlaybackStopped) {
        unlistenEffectPlaybackStopped();
        unlistenEffectPlaybackStopped = null;
    }
    if (unlistenDisplayWindowMoved) {
        unlistenDisplayWindowMoved();
        unlistenDisplayWindowMoved = null;
    }
    cleanupGlobalMenuListeners();
    unregisterEscEffectStopListeners();
    if (displayWindowMoveSaveTimer !== null) {
        clearTimeout(displayWindowMoveSaveTimer);
        displayWindowMoveSaveTimer = null;
    }
    if (spinInterval) {
        clearInterval(spinInterval);
        spinInterval = null;
    }
    window.speechSynthesis.cancel();
    if (spinAudio) { spinAudio.pause(); spinAudio = null; }

    appWindow.outerPosition()
        .then((position) => saveDisplayWindowPosition(position.x, position.y))
        .catch((error) => console.error('Display Window Position Read Error:', error));
});
</script>

<template>
    <div class="bingo-view-container" @mousedown="startWindowDrag" @contextmenu.prevent="handleContextMenu"
        @click="closeMenu">

        <img :src="backgroundImagePath" class="card-bg-img" @error="resetBackgroundImageToFallback" />

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
                    <img v-if="hitNumbers.includes(n)" :src="hitMarkImagePath" class="stamp" @error="resetHitMarkImageToFallback" :style="{
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
                <div class="menu-item" @click="openEffectWindow">🎬 エフェクト画面を表示</div>
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
    top: 4%;
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
    /* 指定の背景色に変更 */
    background: #13284B;
    /* 数字の色に合わせた枠線を追加 */
    /* border: 2px solid #F4B966; */
    /* わずかに透過させるとカードに馴染みます */
    opacity: 0.9;
}

.roulette-number {
    font-size: 2.5rem;
    line-height: 1;
    height: 100%;
    color: #f1c40f;
    font-weight: bold;
    text-shadow: 2px 2px 0 #000;
    /* 指定の文字色に変更 */
    color: #F4B966;
    /* シンプルな配色にするため影は削除か、同系色に */
    text-shadow: none;
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