<script setup lang="ts">
import { ref, watch, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';

/** --- 1. 型定義 (TypeScriptエラーの解消) --- */
interface GridConfig {
    [key: string]: any; // インデックスシグネチャを追加
    x: number; y: number; w: number; h: number; hit_scale: number;
    se_enabled: boolean; se_volume: number;
    tts_enabled: boolean; tts_volume: number; tts_repeat_count: number;
}

/** --- 2. 状態管理 --- */
const grid = ref<GridConfig>({
    x: 22, y: 109, w: 237, h: 239, hit_scale: 100,
    se_enabled: true, se_volume: 20,
    tts_enabled: true, tts_volume: 40, tts_repeat_count: 1
});
const tempGrid = ref<GridConfig>({ ...grid.value });

const hitHistory = ref<number[]>([]);
const redoStack = ref<number[]>([]);
const currentFile = ref<string | null>(null);

const isLive = ref(false);        // 本番モード（書き込み許可）
const isAnimating = ref(false);   // スピン中ガード
const isToggleOpen = ref(false);  // 設定開閉
const isEditing = ref(false);     // 位置調整モード
const sessionFiles = ref<string[]>([]);

/** --- 3. ライフサイクル & 受信イベント --- */
onMounted(async () => {
    try {
        const saved = await invoke<any>('load_settings');
        grid.value = { ...grid.value, ...saved };
        tempGrid.value = { ...grid.value };
        emit('grid-update', grid.value);

        await refreshSessionList();

        // 演出完了時の履歴確定
        await listen<{ number: number }>('bingo-animation-finished', async (event) => {
            if (!isLive.value) return;
            hitHistory.value.push(event.payload.number);
            isAnimating.value = false;
            await persistHits(); // 確定した瞬間に現在のファイルに上書き保存
        });
    } catch (e) { console.error(e); }
});

/** --- 4. セッション管理 --- */
const refreshSessionList = async () => {
    sessionFiles.value = await invoke<string[]>('get_sessions');
};

const startNewBingo = async () => {
    if (hitHistory.value.length > 0 && !confirm("現在の履歴を破棄して新規開始しますか？")) return;
    hitHistory.value = [];
    redoStack.value = [];
    currentFile.value = null; // 次の保存で新規作成
    isLive.value = true;
    emit('bingo-reset', {});
};

const previewSession = async (filename: string) => {
    if (!filename) return;
    const hits = await invoke<number[]>('load_session', { filename });
    hitHistory.value = hits;
    currentFile.value = filename;
    isLive.value = false; // 閲覧モード
    syncBingoCard();
};

const activateLiveMode = () => { isLive.value = true; };

const persistHits = async () => {
    if (!isLive.value) return;
    try {
        const confirmedFile = await invoke<string>('save_session', {
            filename: currentFile.value,
            hits: hitHistory.value
        });
        currentFile.value = confirmedFile;
        await refreshSessionList();
    } catch (e) { console.error("Save Error:", e); }
};

const syncBingoCard = () => { emit('bingo-sync-hits', { hits: [...hitHistory.value] }); };

/** --- 5. ビンゴ操作 --- */
const spin = () => {
    if (!isLive.value || isAnimating.value) return;
    const available = Array.from({ length: 25 }, (_, i) => i + 1).filter(n => !hitHistory.value.includes(n));
    if (available.length === 0) return alert("完売しました！");

    redoStack.value = [];
    const num = available[Math.floor(Math.random() * available.length)];
    isAnimating.value = true;
    emit('bingo-hit', { number: num });
};

const undo = async () => {
    if (!isLive.value || hitHistory.value.length === 0 || isAnimating.value) return;
    const last = hitHistory.value.pop();
    if (last) redoStack.value.push(last);
    syncBingoCard();
    await persistHits();
};

const redo = async () => {
    if (!isLive.value || redoStack.value.length === 0 || isAnimating.value) return;
    const last = redoStack.value.pop();
    if (last) hitHistory.value.push(last);
    syncBingoCard();
    await persistHits();
};

const resetBingo = () => {
    if (confirm("現在のセッションを終了してリセットしますか？")) {
        hitHistory.value = [];
        currentFile.value = null;
        isLive.value = false;
        emit('bingo-reset', {});
    }
};

/** --- 6. 設定調整 --- */
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
                <select @change="e => previewSession((e.target as HTMLSelectElement).value)">
                    <option value="">過去ログを表示（閲覧モード）</option>
                    <option v-for="f in sessionFiles" :key="f" :value="f">{{ f }}</option>
                </select>
                <button v-if="currentFile && !isLive" class="btn-resume" @click="activateLiveMode">▶ 本番として続行</button>
            </div>
            <div v-if="currentFile" class="status-bar">
                📄: {{ currentFile }} <span v-if="isLive" class="live-tag">LIVE</span>
            </div>
        </section>

        <section class="main-mgr">
            <button class="spin-btn" :disabled="!isLive || isAnimating" @click="spin">
                <template v-if="!isLive">⚠️ 閲覧中（スピン不可）</template>
                <template v-else>{{ isAnimating ? '抽選中...' : 'SPIN BINGO' }}</template>
            </button>
            <div class="step-actions">
                <button :disabled="!isLive || hitHistory.length === 0 || isAnimating" @click="undo">Undo</button>
                <button :disabled="!isLive || redoStack.length === 0 || isAnimating" @click="redo">Redo</button>
                <button class="btn-reset" @click="resetBingo">RESET</button>
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
                        <label><input type="checkbox" v-model="tempGrid.se_enabled"> SE有効</label>
                        <div class="slider-row">
                            <input type="range" min="0" max="100" v-model.number="tempGrid.se_volume">
                            <span>{{ tempGrid.se_volume }}%</span>
                        </div>
                    </div>
                    <div class="setting-item">
                        <label><input type="checkbox" v-model="tempGrid.tts_enabled"> 読み上げ有効</label>
                        <div class="slider-row">
                            <input type="range" min="0" max="100" v-model.number="tempGrid.tts_volume">
                            <span>{{ tempGrid.tts_volume }}%</span>
                        </div>
                        <select v-if="tempGrid.tts_enabled" v-model.number="tempGrid.tts_repeat_count">
                            <option v-for="i in 3" :key="i" :value="i">{{ i }}回</option>
                        </select>
                    </div>
                </div>

                <hr class="divider" />

                <button v-if="!isEditing" class="btn-edit-trigger" @click="startEdit">📏 位置調整を開始</button>

                <div v-if="isEditing" class="editing-ui">
                    <div class="sliders">
                        <div v-for="key in (['x', 'y', 'w', 'h'] as const)" :key="key" class="slider-item">
                            <label>{{ key.toUpperCase() }}</label>
                            <input type="range" min="0" max="400" v-model.number="tempGrid[key]">
                            <span>{{ tempGrid[key] }}px</span>
                        </div>
                        <div class="slider-item highlight">
                            <label>縮尺</label>
                            <input type="range" min="10" max="200" v-model.number="tempGrid.hit_scale">
                            <span>{{ tempGrid.hit_scale }}%</span>
                        </div>
                    </div>
                    <div class="edit-footer">
                        <button class="btn-save" @click="confirmEdit">保存</button>
                        <button class="btn-cancel" @click="cancelEdit">破棄</button>
                    </div>
                </div>
            </div>
        </section>
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
    font-size: 1.1rem;
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

.load-group {
    display: flex;
    gap: 8px;
    margin-top: 10px;
}

.load-group select {
    flex: 1;
    background: #2c3e50;
    color: white;
    border: 1px solid #444;
    border-radius: 4px;
}

.btn-resume {
    background: #f39c12;
    color: white;
    border: none;
    padding: 0 10px;
    border-radius: 4px;
    cursor: pointer;
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
    cursor: not-allowed;
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

.btn-reset {
    background: #c0392b !important;
}

.history-view h4 {
    margin: 0 0 8px 0;
    font-size: 0.85rem;
    color: #bdc3c7;
}

.tag-cloud {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
}

.tag {
    background: #f1c40f;
    color: #2c3e50;
    padding: 2px 8px;
    border-radius: 10px;
    font-size: 0.8rem;
    font-weight: bold;
}

.accordion-head {
    display: flex;
    justify-content: space-between;
    cursor: pointer;
    font-weight: bold;
    font-size: 0.9rem;
}

.slider-row {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 5px;
}

.slider-row span {
    width: 3em;
    text-align: right;
    font-family: monospace;
}

.divider {
    border: 0;
    border-top: 1px solid rgba(255, 255, 255, 0.1);
    margin: 15px 0;
}

.btn-edit-trigger {
    width: 100%;
    padding: 8px;
    background: #3498db;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
}

.slider-item {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;
    font-size: 0.8rem;
}

.slider-item label {
    width: 2em;
}

.slider-item input {
    flex: 1;
}

.slider-item span {
    width: 4em;
    text-align: right;
}

.highlight {
    color: #f1c40f;
    font-weight: bold;
}

.edit-footer {
    display: flex;
    gap: 8px;
    margin-top: 12px;
}

.btn-save {
    flex: 2;
    padding: 8px;
    background: #27ae60;
    color: white;
    border: none;
    border-radius: 4px;
}

.btn-cancel {
    flex: 1;
    padding: 8px;
    background: #95a5a6;
    color: white;
    border: none;
    border-radius: 4px;
}
</style>