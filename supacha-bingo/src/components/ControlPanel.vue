<script setup lang="ts">
import { ref, watch, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';

/** ----------------------------------------------------------------
 * 1. 状態管理 (States)
 * ---------------------------------------------------------------- */
const grid = ref({ x: 22, y: 109, w: 237, h: 239, hit_scale: 100, se_enabled: true, se_volume: 20, tts_enabled: true, tts_volume: 40, tts_repeat_count: 1 });
const tempGrid = ref({ ...grid.value });

const hitHistory = ref<number[]>([]);
const redoStack = ref<number[]>([]);
const currentFile = ref<string | null>(null);

const isLive = ref(false);        // 本番モードか（書き込み許可）
const isAnimating = ref(false);   // スピン中か
const isToggleOpen = ref(false);  // 設定アコーディオン
const isEditing = ref(false);     // グリッド編集中か
const sessionFiles = ref<string[]>([]);

/** ----------------------------------------------------------------
 * 2. ライフサイクル & イベント受信
 * ---------------------------------------------------------------- */
onMounted(async () => {
    const saved = await invoke<any>('load_settings');
    grid.value = { ...grid.value, ...saved };
    tempGrid.value = { ...grid.value };
    emit('grid-update', grid.value);

    await refreshSessionList();

    // 演出終了通知を受けたときの最終処理
    await listen<{ number: number }>('bingo-animation-finished', async (event) => {
        if (!isLive.value) return;
        hitHistory.value.push(event.payload.number);
        isAnimating.value = false;
        await persistHits(); // 確定した瞬間にファイル保存
    });
});

/** ----------------------------------------------------------------
 * 3. セッション管理ロジック (Session Management)
 * ---------------------------------------------------------------- */
const refreshSessionList = async () => { sessionFiles.value = await invoke('get_sessions'); };

// 新規開始
const startNewBingo = async () => {
    if (hitHistory.value.length > 0 && !confirm("現在の履歴を破棄して新規開始しますか？")) return;
    hitHistory.value = [];
    redoStack.value = [];
    currentFile.value = null; // 次の保存で新規作成
    isLive.value = true;
    emit('bingo-reset', {});
};

// 過去ログを「見る」
const previewSession = async (filename: string) => {
    if (!filename) return;
    const hits = await invoke<number[]>('load_session', { filename });
    hitHistory.value = hits;
    currentFile.value = filename;
    isLive.value = false; // 閲覧モード
    syncBingoCard();
};

// 「本番として再開」
const activateLiveMode = () => { isLive.value = true; };

// 保存処理の集約
const persistHits = async () => {
    if (!isLive.value) return;
    const res = await invoke<string>('save_session', { filename: currentFile.value, hits: hitHistory.value });
    currentFile.value = res;
    await refreshSessionList();
};

const syncBingoCard = () => { emit('bingo-sync-hits', { hits: [...hitHistory.value] }); };

/** ----------------------------------------------------------------
 * 4. メイン操作 (Bingo Actions)
 * ---------------------------------------------------------------- */
const spin = () => {
    if (!isLive.value || isAnimating.value) return;
    const available = Array.from({ length: 25 }, (_, i) => i + 1).filter(n => !hitHistory.value.includes(n));
    if (available.length === 0) return alert("完売です！");

    redoStack.value = []; // 新しい操作をしたらRedoは消去
    const num = available[Math.floor(Math.random() * available.length)];
    isAnimating.value = true;
    emit('bingo-hit', { number: num });
};

const undo = async () => {
    if (!isLive.value || hitHistory.value.length === 0) return;
    const last = hitHistory.value.pop();
    if (last) redoStack.value.push(last);
    syncBingoCard();
    await persistHits();
};

/** ----------------------------------------------------------------
 * 5. 各種設定ロジック (Settings)
 * ---------------------------------------------------------------- */
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
        <h2 class="title">🎡 Bingo Operation</h2>

        <section class="session-section">
            <button class="btn-primary" @click="startNewBingo">✨ 新規開始</button>
            <div class="load-group">
                <select @change="e => previewSession((e.target as HTMLSelectElement).value)">
                    <option value="">過去ログを表示...</option>
                    <option v-for="f in sessionFiles" :value="f">{{ f }}</option>
                </select>
                <button v-if="currentFile && !isLive" class="btn-resume" @click="activateLiveMode">▶ 本番として続行</button>
            </div>
            <div v-if="currentFile" class="status-bar">
                ファイル: {{ currentFile }} <span v-if="isLive" class="live-tag">LIVE</span>
            </div>
        </section>

        <section class="main-section">
            <button class="spin-btn" :disabled="!isLive || isAnimating" @click="spin">
                <span v-if="!isLive">⚠️ 閲覧中</span>
                <span v-else>{{ isAnimating ? '抽選中...' : 'SPIN BINGO' }}</span>
            </button>
            <div class="history-controls">
                <button :disabled="!isLive || hitHistory.length === 0" @click="undo">Undo</button>
                <button :disabled="!isLive || redoStack.length === 0" @click="redo">Redo</button>
                <button class="btn-reset" @click="startNewBingo">RESET</button>
            </div>
        </section>

        <section class="history-view">
            <h4>当選履歴 ({{ hitHistory.length }} / 25)</h4>
            <div class="tag-container">
                <span v-for="n in hitHistory" :key="n" class="tag">{{ n }}</span>
            </div>
        </section>

        <section class="settings-section">
            <div class="accordion-header" @click="isToggleOpen = !isToggleOpen">
                <span>⚙️ 各種設定</span>
                <span>{{ isToggleOpen ? '▲' : '▼' }}</span>
            </div>

            <div v-if="isToggleOpen" class="accordion-body">
                <div class="setting-group">
                    <label><input type="checkbox" v-model="tempGrid.se_enabled"> 効果音</label>
                    <div class="range-box">
                        <input type="range" min="0" max="100" v-model.number="tempGrid.se_volume">
                        <span>{{ tempGrid.se_volume }}%</span>
                    </div>
                </div>
                <div class="setting-group">
                    <label><input type="checkbox" v-model="tempGrid.tts_enabled"> 読み上げ</label>
                    <div class="range-box">
                        <input type="range" min="0" max="100" v-model.number="tempGrid.tts_volume">
                        <span>{{ tempGrid.tts_volume }}%</span>
                    </div>
                    <select v-if="tempGrid.tts_enabled" v-model.number="tempGrid.tts_repeat_count">
                        <option v-for="i in 3" :value="i">{{ i }}回</option>
                    </select>
                </div>

                <hr />

                <button v-if="!isEditing" class="btn-edit" @click="startEdit">📏 位置調整を開始</button>
                <div v-else class="edit-ui">
                    <div class="sliders">
                        <div v-for="key in ['x', 'y', 'w', 'h']" :key="key" class="slider-row">
                            <label>{{ key.toUpperCase() }}</label>
                            <input type="range" :min="0" :max="400" v-model.number="tempGrid[key]">
                            <span>{{ tempGrid[key] }}px</span>
                        </div>
                        <div class="slider-row">
                            <label>縮尺</label>
                            <input type="range" min="10" max="200" v-model.number="tempGrid.hit_scale">
                            <span>{{ tempGrid.hit_scale }}%</span>
                        </div>
                    </div>
                    <div class="edit-btns">
                        <button class="btn-save" @click="confirmEdit">保存</button>
                        <button class="btn-cancel" @click="cancelEdit">破棄</button>
                    </div>
                </div>
            </div>
        </section>
    </div>
</template>

<style scoped>
/* スタイルも論理的に整理 */
.panel {
    padding: 15px;
    background: #1a2a3a;
    color: #eee;
    height: 100vh;
    overflow-y: auto;
    font-family: sans-serif;
}

.title {
    font-size: 1.1rem;
    border-bottom: 1px solid #34495e;
    padding-bottom: 8px;
    margin-bottom: 15px;
}

section {
    background: rgba(0, 0, 0, 0.2);
    border-radius: 6px;
    padding: 12px;
    margin-bottom: 15px;
    border: 1px solid rgba(255, 255, 255, 0.05);
}

.session-section .btn-primary {
    width: 100%;
    padding: 10px;
    background: #27ae60;
    border: none;
    color: white;
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
    padding: 5px;
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
    height: 65px;
    background: #e74c3c;
    color: white;
    border: none;
    border-radius: 6px;
    font-size: 1.3rem;
    font-weight: bold;
    cursor: pointer;
    transition: 0.2s;
}

.spin-btn:disabled {
    background: #7f8c8d;
    cursor: not-allowed;
    opacity: 0.6;
}

.history-controls {
    display: flex;
    gap: 8px;
    margin-top: 10px;
}

.history-controls button {
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
}

.tag-container {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
}

.tag {
    background: #f1c40f;
    color: #2c3e50;
    padding: 2px 7px;
    border-radius: 10px;
    font-weight: bold;
    font-size: 0.8rem;
}

.accordion-header {
    display: flex;
    justify-content: space-between;
    cursor: pointer;
    padding: 5px 0;
    font-weight: bold;
    font-size: 0.9rem;
}

.setting-group {
    margin-bottom: 12px;
}

.setting-group label {
    display: block;
    font-size: 0.8rem;
    margin-bottom: 4px;
    color: #3498db;
}

.range-box {
    display: flex;
    align-items: center;
    gap: 10px;
}

.range-box span {
    width: 3em;
    font-family: monospace;
    font-size: 0.9rem;
    text-align: right;
}

.btn-edit {
    width: 100%;
    padding: 8px;
    background: #3498db;
    border: none;
    color: white;
    border-radius: 4px;
    cursor: pointer;
}

.slider-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
    font-size: 0.8rem;
}

.slider-row label {
    width: 2em;
    margin: 0;
}

.slider-row input {
    flex: 1;
}

.slider-row span {
    width: 4em;
    text-align: right;
}

.edit-btns {
    display: flex;
    gap: 8px;
    margin-top: 10px;
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