<script setup lang="ts">
import { ref, watch, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import BingoModal from './BingoModal.vue';

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

        await listen<{ number: number }>('bingo-animation-finished', async (event) => {
            if (!isLive.value) return;
            if (!hitHistory.value.includes(event.payload.number)) {
                hitHistory.value.push(event.payload.number);
            }
            isAnimating.value = false;
            await persistHits('HIT');
        });
    } catch (e) { console.error("Initialize Error:", e); }
});

const refreshSessionList = async () => {
    sessionFiles.value = await invoke<string[]>('get_sessions');
};

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
        currentFile.value = null;
        hitHistory.value = [];
        redoStack.value = [];
        isLive.value = false;
        emit('bingo-reset', {});
        await invoke('log_action', { trigger: 'CLOSE', message: 'プレビュー終了' });
        return;
    }
    const hits = await invoke<number[]>('load_session', { filename });
    hitHistory.value = hits;
    currentFile.value = filename;
    isLive.value = false;
    emit('bingo-sync-hits', { hits: [...hits] });
};

const persistHits = async (trigger: string = 'AUTO') => {
    if (!isLive.value) return;
    try {
        const confirmedFile = await invoke<string>('save_session', {
            filename: currentFile.value,
            hits: hitHistory.value,
            trigger: trigger
        });
        currentFile.value = confirmedFile;
        await refreshSessionList();
    } catch (e) { console.error("Save Error:", e); }
};

/** --- 4. ビンゴ操作 --- */
const getSecureRandomInt = (max: number): number => {
    const array = new Uint32Array(1);
    const range = 0xFFFFFFFF;
    const limit = range - (range % max);
    let val: number;
    do {
        window.crypto.getRandomValues(array);
        val = array[0];
    } while (val >= limit);
    return val % max;
};

const spin = () => {
    if (!isLive.value || isAnimating.value) return;
    const available = Array.from({ length: 25 }, (_, i) => i + 1).filter(n => !hitHistory.value.includes(n));
    if (available.length === 0) return modal.value?.show("番号はすべて選出しました！", "alert");

    const randomIndex = getSecureRandomInt(available.length);
    const num = available[randomIndex];

    redoStack.value = [];
    isAnimating.value = true;
    emit('bingo-hit', { number: num });
};

const undo = async () => {
    if (!isLive.value || hitHistory.value.length === 0) return;
    const last = hitHistory.value.pop();
    if (last) redoStack.value.push(last);
    emit('bingo-sync-hits', { hits: [...hitHistory.value] });
    await persistHits('UNDO');
};

const redo = async () => {
    if (!isLive.value || redoStack.value.length === 0) return;
    const last = redoStack.value.pop();
    if (last) hitHistory.value.push(last);
    emit('bingo-sync-hits', { hits: [...hitHistory.value] });
    await persistHits('REDO');
};

const resetBingo = async () => {
    const confirmed = await modal.value?.show("履歴をリセットし、\nセッションを終了しますか？", "confirm");
    if (confirmed) {
        hitHistory.value = [];
        currentFile.value = null;
        redoStack.value = [];
        isLive.value = false;
        emit('bingo-reset', {});
        await invoke('log_action', { trigger: 'RESET', message: 'セッション初期化' });
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
                        <div class="setting-header">
                            <label class="item-label">SE音量: {{ tempGrid.se_volume }}%</label>
                            <button class="toggle-btn" :class="{ 'is-active': tempGrid.se_enabled }"
                                @click="tempGrid.se_enabled = !tempGrid.se_enabled">
                                {{ tempGrid.se_enabled ? 'ON' : 'OFF' }}
                            </button>
                        </div>
                        <input type="range" min="0" max="100" v-model.number="tempGrid.se_volume" class="custom-slider"
                            :disabled="!tempGrid.se_enabled">
                    </div>

                    <div class="setting-item">
                        <div class="setting-header">
                            <label class="item-label">TTS音量: {{ tempGrid.tts_volume }}%</label>
                            <button class="toggle-btn" :class="{ 'is-active': tempGrid.tts_enabled }"
                                @click="tempGrid.tts_enabled = !tempGrid.tts_enabled">
                                {{ tempGrid.tts_enabled ? 'ON' : 'OFF' }}
                            </button>
                        </div>
                        <input type="range" min="0" max="100" v-model.number="tempGrid.tts_volume" class="custom-slider"
                            :disabled="!tempGrid.tts_enabled">
                    </div>

                    <div v-if="tempGrid.tts_enabled" class="setting-item row-layout">
                        <label class="item-label no-margin">読み上げ回数</label>
                        <select v-model.number="tempGrid.tts_repeat_count" class="tts-select compact">
                            <option v-for="i in 3" :key="i" :value="i">{{ i }}回</option>
                        </select>
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

/* ボタン類 */
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
    color: white;
    border: 1px solid #a93226;
    font-weight: bold;
    transition: all 0.2s;
}

.btn-danger:hover:not(:disabled) {
    background: #e74c3c !important;
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
}

.load-group select {
    flex: 1;
    background: #2c3e50;
    color: white;
    border-radius: 4px;
    border: 1px solid #444;
    min-width: 0;
    text-overflow: ellipsis;
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
    filter: grayscale(1);
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

/* 設定項目 */
.setting-item {
    margin-bottom: 15px;
    display: flex;
    flex-direction: column;
    gap: 5px;
}

.setting-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 5px;
}

/* 読み上げ回数（横並び） */
.row-layout {
    flex-direction: row !important;
    align-items: center;
    gap: 4px;
    margin-top: 5px;
}

.no-margin {
    margin-right: 2px;
}

.item-label {
    font-size: 0.85rem;
    color: #3498db;
    display: flex;
    align-items: center;
}

/* トグルボタン */
.toggle-btn {
    width: 60px;
    height: 24px;
    border: 1px solid #444;
    border-radius: 12px;
    background: #2c3e50;
    color: #7f8c8d;
    font-size: 0.7rem;
    font-weight: bold;
    cursor: pointer;
    transition: all 0.2s ease;
    display: flex;
    align-items: center;
    justify-content: center;
}

.toggle-btn.is-active {
    background: #27ae60;
    color: white;
    border-color: #2ecc71;
    box-shadow: 0 0 8px rgba(46, 204, 113, 0.4);
}

/* プルダウン */
.tts-select.compact {
    background: #2c3e50;
    color: white;
    border: 1px solid #444;
    border-radius: 4px;
    padding: 2px 4px;
    cursor: pointer;
    font-size: 0.8rem;
    min-width: 60px;
    height: 24px;
    transition: all 0.2s;
}

.tts-select.compact:hover:not(:disabled) {
    border-color: #3498db;
    background: #34495e;
}

.tts-select.compact:focus {
    outline: none;
    border-color: #3498db;
    box-shadow: 0 0 8px rgba(52, 152, 219, 0.4);
}

/* スライダー */
.custom-slider {
    appearance: none;
    -webkit-appearance: none;
    width: 100%;
    height: 6px;
    background: #2c3e50;
    border-radius: 3px;
    outline: none;
}

.custom-slider:disabled {
    opacity: 0.3;
}

.custom-slider::-webkit-slider-thumb {
    appearance: none;
    -webkit-appearance: none;
    width: 18px;
    height: 18px;
    background: #3498db;
    border: 2px solid #eee;
    border-radius: 50%;
    cursor: pointer;
    box-shadow: 0 0 5px rgba(0, 0, 0, 0.5);
}

.custom-slider:not(:disabled)::-webkit-slider-thumb:hover {
    transform: scale(1.1);
    background: #2980b9;
}

.slider-row {
    margin-bottom: 12px;
    display: flex;
    flex-direction: column;
    gap: 5px;
}

.slider-label {
    font-size: 0.85rem;
}

.highlight {
    color: #f1c40f;
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