<script setup lang="ts">
import { ref, watch, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import BingoModal from './BingoModal.vue'; // インポート

// ダイアログの参照
const modal = ref<InstanceType<typeof BingoModal> | null>(null);

/** --- 1. 型定義と初期状態 --- */
interface GridConfig {
    [key: string]: any;
    x: number; y: number; w: number; h: number; hit_scale: number;
    se_enabled: boolean; se_volume: number;
    tts_enabled: boolean; tts_volume: number; tts_repeat_count: number;
}

const grid = ref<GridConfig>({
    x: 22, y: 109, w: 237, h: 239, hit_scale: 100,
    se_enabled: true, se_volume: 50, tts_enabled: true, tts_volume: 50, tts_repeat_count: 1
});
const tempGrid = ref<GridConfig>({ ...grid.value });
const hitHistory = ref<number[]>([]);
const redoStack = ref<number[]>([]);
const currentFile = ref<string | null>(null);
const isLive = ref(false);
const isAnimating = ref(false);
const isToggleOpen = ref(false);
const isEditing = ref(false);
const sessionFiles = ref<string[]>([]);

/** --- 2. ライフサイクル --- */
onMounted(async () => {
    try {
        const saved = await invoke<any>('load_settings');
        grid.value = { ...grid.value, ...saved };
        tempGrid.value = { ...grid.value };
        emit('grid-update', grid.value);
        await refreshSessionList();

        // 演出完了時の処理
        await listen<{ number: number }>('bingo-animation-finished', async (event) => {
            if (!isLive.value) return;
            if (!hitHistory.value.includes(event.payload.number)) {
                hitHistory.value.push(event.payload.number);
            }
            isAnimating.value = false;
            await persistHits('HIT'); // 抽選ヒットによる保存
        });
    } catch (e) { console.error(e); }
});

const refreshSessionList = async () => { sessionFiles.value = await invoke<string[]>('get_sessions'); };

/** --- 3. セッション管理 --- */
const startNewBingo = async () => {
    if (hitHistory.value.length > 0) {
        const confirmed = await modal.value?.show("現在の履歴から再開せず\n新規開始しますか？", "confirm");
        if (!confirmed) return;
    }
    hitHistory.value = [];
    redoStack.value = [];
    currentFile.value = null;
    isLive.value = true;
    emit('bingo-reset', {});
};

const previewSession = async (filename: string) => {
    if (!filename) {
        // 未選択（空）にした際の状態クリア
        currentFile.value = null;
        hitHistory.value = [];
        redoStack.value = [];
        isLive.value = false;
        emit('bingo-reset', {});

        // アクションログを記録
        await invoke('log_action', {
            trigger: 'CLOSE',
            message: 'プレビューを終了し、未選択状態に戻りました。'
        });
        return;
    }

    // 既存の読み込み処理
    const hits = await invoke<number[]>('load_session', { filename });
    hitHistory.value = hits;
    currentFile.value = filename;
    isLive.value = false;
    emit('bingo-sync-hits', { hits: [...hits] });
};

// 引数 trigger を追加し、デフォルト値を 'AUTO' に設定
const persistHits = async (trigger: string = 'AUTO') => {
    if (!isLive.value) return;
    try {
        const confirmedFile = await invoke<string>('save_session', {
            filename: currentFile.value,
            hits: hitHistory.value,
            trigger: trigger // ここで Rust 側に契機を送信
        });
        currentFile.value = confirmedFile;
        await refreshSessionList();
    } catch (e) {
        console.error("Save Error:", e);
    }
};

/** --- 4. ビンゴ操作 --- */
/**
 * 指定された範囲 [0, max) で、暗号学的に安全な乱数を生成する
 * 剰余による偏り（Modulo Bias）を排除した実装
 */
const getSecureRandomInt = (max: number): number => {
    const array = new Uint32Array(1);
    const range = 0xFFFFFFFF; // 32ビット整数の最大値
    const limit = range - (range % max); // 偏りを生む余り部分を除外する境界

    let val: number;
    do {
        window.crypto.getRandomValues(array);
        val = array[0];
    } while (val >= limit); // 境界を超えた場合は再生成（リジェクション・サンプリング）

    return val % max;
};
const spin = () => {
    if (!isLive.value || isAnimating.value) return;
    const available = Array.from({ length: 25 }, (_, i) => i + 1).filter(n => !hitHistory.value.includes(n));
    if (available.length === 0) return modal.value?.show("番号はすべて選出しました！", "alert");
    redoStack.value = [];
    // --- ガチな選出 ---
    // 配列の長さに基づいたインデックスをセキュアに選出
    const randomIndex = getSecureRandomInt(available.length);
    const num = available[randomIndex];
    // ------------------

    redoStack.value = [];
    isAnimating.value = true;
    emit('bingo-hit', { number: num });
};

const undo = async () => {
    if (!isLive.value || hitHistory.value.length === 0) return;
    const last = hitHistory.value.pop();
    if (last) redoStack.value.push(last);
    emit('bingo-sync-hits', { hits: [...hitHistory.value] });
    await persistHits('UNDO'); // 元に戻す操作による保存
};

const redo = async () => { // 欠落していた関数を再定義
    if (!isLive.value || redoStack.value.length === 0) return;
    const last = redoStack.value.pop();
    if (last) hitHistory.value.push(last);
    emit('bingo-sync-hits', { hits: [...hitHistory.value] });
    await persistHits('REDO'); // やり直す操作による保存
};

const resetBingo = async () => {
    const confirmed = await modal.value?.show("履歴をリセットし、\nセッションを終了しますか？", "confirm");
    if (confirmed) {
        hitHistory.value = [];
        currentFile.value = null; // ここで null にすることで UI も未選択に戻る
        redoStack.value = [];
        isLive.value = false;
        emit('bingo-reset', {});

        // アクションログを記録
        await invoke('log_action', {
            trigger: 'RESET',
            message: '履歴を完全にクリアし、セッションを初期状態に戻しました。'
        });
    }
};

/** --- 5. 設定管理 --- */
watch(tempGrid, (val) => { emit('grid-update', { ...val }); }, { deep: true });
watch(isEditing, (val) => { emit('edit-mode-update', val); });

const startEdit = () => { isEditing.value = true; };
const confirmEdit = async () => {
    grid.value = { ...tempGrid.value };
    await invoke('save_settings', { config: grid.value });
    isEditing.value = false;
};
const cancelEdit = () => {
    tempGrid.value = { ...grid.value };
    isEditing.value = false;
    emit('grid-update', grid.value);
};
</script>

<template>
    <div class="panel">
        <h3 class="header">🎡 Bingo Operation</h3>
        <section class="session-mgr">
            <button class="btn-primary" @click="startNewBingo">✨ 新規開始</button>
            <div class="load-group">
                <select :value="currentFile || ''" @change="e => previewSession((e.target as HTMLSelectElement).value)">
                    <option value="">過去ログを表示（閲覧）</option>
                    <option v-for="f in sessionFiles" :key="f" :value="f">{{ f }}</option>
                </select>
                <button v-if="currentFile && !isLive" class="btn-resume" @click="isLive = true">▶ 再開</button>
            </div>
            <div v-if="currentFile" class="status-bar">
                📄: {{ currentFile }} <span v-if="isLive" class="live-tag">LIVE</span>
            </div>
        </section>

        <section class="main-mgr">
            <button class="spin-btn" :disabled="!isLive || isAnimating" @click="spin">
                <template v-if="!isLive">⚠️ 閲覧モード</template>
                <template v-else>{{ isAnimating ? '抽選中...' : '！抽選開始！' }}</template>
            </button>
            <div class="step-actions">
                <button :disabled="!isLive || hitHistory.length === 0" @click="undo"
                    :class="{ 'is-disabled': hitHistory.length === 0 }">前に戻す</button>
                <button :disabled="!isLive || redoStack.length === 0" @click="redo"
                    :class="{ 'is-disabled': redoStack.length === 0 }">次に進む</button>
                <button class="btn-reset btn-danger" :disabled="!isLive && hitHistory.length === 0"
                    @click="resetBingo">リセット</button>
            </div>
        </section>

        <section class="history-view">
            <h4>当選履歴 ({{ hitHistory.length }} / 25)</h4>
            <div class="tag-cloud">
                <span v-for="n in hitHistory" :key="n" class="tag">{{ n }}</span>
            </div>
        </section>

        <section class="settings-mgr">
            <div class="accordion-head" @click="isToggleOpen = !isToggleOpen">
                <span>⚙️ 設定 (位置・音響)</span>
                <span>{{ isToggleOpen ? '▲' : '▼' }}</span>
            </div>
            <div v-if="isToggleOpen" class="accordion-body">
                <div class="audio-group">
                    <div class="setting-item">
                        <label class="item-label">
                            <input type="checkbox" v-model="tempGrid.se_enabled"> SE音量: {{ tempGrid.se_volume }}%
                        </label>
                        <input type="range" min="0" max="100" v-model.number="tempGrid.se_volume"
                            :disabled="!tempGrid.se_enabled">
                    </div>
                    <div class="setting-item">
                        <label class="item-label">
                            <input type="checkbox" v-model="tempGrid.tts_enabled"> TTS音量: {{ tempGrid.tts_volume }}%
                        </label>
                        <div class="tts-row">
                            <input type="range" min="0" max="100" v-model.number="tempGrid.tts_volume"
                                :disabled="!tempGrid.tts_enabled">
                        </div>
                        <div v-if="tempGrid.tts_enabled" class="setting-item">
                            <label class="item-label">読み上げ回数:</label>
                            <select v-model.number="tempGrid.tts_repeat_count" class="tts-select full-width">
                                <option v-for="i in 3" :key="i" :value="i">{{ i }}回読み上げ</option>
                            </select>
                        </div>
                    </div>
                </div>
                <hr class="divider" />
                <button v-if="!isEditing" class="btn-edit" @click="startEdit">📏 位置調整開始</button>
                <div v-else class="editing-ui">
                    <div class="sliders">
                        <div v-for="key in (['x', 'y', 'w', 'h'] as const)" :key="key" class="slider-row">
                            <label class="slider-label">{{ key.toUpperCase() }}: {{ tempGrid[key] }}px</label>
                            <input type="range" min="0" max="400" v-model.number="tempGrid[key]">
                        </div>
                        <div class="slider-row highlight">
                            <label class="slider-label">スタンプ縮尺: {{ tempGrid.hit_scale }}%</label>
                            <input type="range" min="10" max="200" v-model.number="tempGrid.hit_scale">
                        </div>
                    </div>
                    <div class="edit-footer">
                        <button class="btn-save" @click="confirmEdit">保存</button>
                        <button class="btn-cancel" @click="cancelEdit">破棄</button>
                    </div>
                </div>
            </div>
        </section>
        <BingoModal ref="modal" />
    </div>
</template>

<style scoped>
/* 既存のスタイルをベースに、スライダー幅を最適化 */
.panel {
    padding: 15px;
    background: #1a2a3a;
    color: #eee;
    height: 100vh;
    overflow-y: auto;
    font-family: sans-serif;
}

.header {
    border-bottom: 1px solid #34495e;
    padding-bottom: 8px;
    margin-bottom: 15px;
}

section {
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 6px;
    padding: 12px;
    margin-bottom: 15px;
}

.btn-primary {
    width: 100%;
    padding: 10px;
    background: #27ae60;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
}

.btn-danger {
    background: #c0392b !important;
    /* 深い赤 */
    color: white;
    border: 1px solid #a93226;
    font-weight: bold;
    transition: all 0.2s;
}

.btn-danger:hover:not(:disabled) {
    background: #e74c3c !important;
    /* ホバーで明るい赤に */
    box-shadow: 0 0 10px rgba(231, 76, 60, 0.5);
}

.btn-danger:disabled {
    background: #2c3e50 !important;
    border-color: #34495e;
    opacity: 0.4;
    cursor: not-allowed;
    filter: grayscale(1);
}

.load-group {
    display: flex;
    gap: 8px;
    margin-top: 10px;
    max-width: 100%;
}

.load-group select {
    flex: 1;
    background: #2c3e50;
    color: white;
    border-radius: 4px;
    border: 1px solid #444;

    min-width: 0;
    width: 100%;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.status-bar {
    font-size: 0.75rem;
    margin-top: 8px;
    color: #bdc3c7;
}

.live-tag {
    background: #e74c3c;
    color: white;
    padding: 1px 4px;
    border-radius: 3px;
    font-weight: bold;
}

.spin-btn {
    width: 100%;
    height: 60px;
    background: #e74c3c;
    color: white;
    border: none;
    border-radius: 6px;
    font-size: 1.2rem;
    font-weight: bold;
    cursor: pointer;
}

.spin-btn:disabled {
    background: #7f8c8d;
    opacity: 0.5;
}

.step-actions {
    display: flex;
    gap: 8px;
    margin-top: 10px;
}

.step-actions button {
    flex: 1;
    padding: 8px;
    background: #34495e;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
}

.step-actions button:disabled {
    opacity: 0.3;
    cursor: not-allowed;
    /* 白黒にして「機能死」を演出 */
    filter: grayscale(1);
}

.history-view h4 {
    margin: 0 0 8px 0;
    font-size: 0.85rem;
}

.tag-cloud {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-top: 8px;
}

.tag {
    background: #f1c40f;
    color: #2c3e50;
    padding: 2px 8px;
    border-radius: 10px;
    font-size: 0.8rem;
    font-weight: bold;
}

/* 【修正】音響・位置調整のレイアウト改善 */
.setting-item {
    margin-bottom: 15px;
    display: flex;
    flex-direction: column;
    gap: 5px;
}

.item-label {
    font-size: 0.85rem;
    color: #3498db;
    display: flex;
    align-items: center;
    gap: 8px;
}

.tts-row {
    display: flex;
    gap: 10px;
    align-items: center;
}

.tts-select {
    background: #2c3e50;
    color: white;
    border: 1px solid #444;
    border-radius: 4px;
    padding: 2px 5px;
}

.slider-row {
    margin-bottom: 12px;
    display: flex;
    flex-direction: column;
    gap: 5px;
}

.slider-label {
    font-size: 0.85rem;
    display: block;
}

.highlight {
    color: #f1c40f;
}

/* スライダーを横いっぱいに広げる */
input[type="range"] {
    width: 100%;
    cursor: pointer;
}

.accordion-head {
    display: flex;
    justify-content: space-between;
    cursor: pointer;
    font-weight: bold;
}

.divider {
    border: 0;
    border-top: 1px solid rgba(255, 255, 255, 0.1);
    margin: 15px 0;
}

.btn-edit {
    width: 100%;
    padding: 8px;
    background: #3498db;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
}

.edit-footer {
    display: flex;
    gap: 8px;
    margin-top: 12px;
}

.btn-save {
    flex: 2;
    background: #27ae60;
    color: white;
    border: none;
    padding: 8px;
    border-radius: 4px;
}

.btn-cancel {
    flex: 1;
    background: #95a5a6;
    color: white;
    border: none;
    padding: 8px;
    border-radius: 4px;
}
</style>