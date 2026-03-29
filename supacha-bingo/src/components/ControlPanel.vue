<script setup lang="ts">
import { ref, watch, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';

// 確定済みの位置情報（表示画面と同期している値）
const grid = ref({
    x: 22, y: 109, w: 237, h: 239, hit_scale: 100,
    se_enabled: true, se_volume: 20,
    tts_enabled: true, tts_volume: 40, tts_repeat_count: 1
});
// 編集中のアコーディオン（トグル）の開閉状態
const isToggleOpen = ref(false);
// 当選履歴
const hitHistory = ref<number[]>([]);
// 【追加】アニメーション中フラグ
const isAnimating = ref(false);

onMounted(async () => {
    try {
        const saved = await invoke<any>('load_settings');
        // 保存された値で上書きマージ
        grid.value = { ...grid.value, ...saved };
        // 【修正】マージ後の完全なオブジェクトを tempGrid に渡す
        tempGrid.value = { ...grid.value };
        emit('grid-update', grid.value);

        // 【追加】アニメーション完了通知を受信して履歴を更新
        await listen<{ number: number }>('bingo-animation-finished', (event) => {
            hitHistory.value.push(event.payload.number);
            isAnimating.value = false; // ガード解除
        });
    } catch (e) {
        console.error("設定の読み込みに失敗しました。デフォルト値を使用します:", e);
        console.error(e);
    }
});

// 編集中の値を一時的に保持する変数
const tempGrid = ref({ ...grid.value });
watch(tempGrid, (newVal) => {
    emit('grid-update', { ...newVal });
}, { deep: true });

// 編集モード（変更ボタン押下後）の状態
const isEditing = ref(false);
// 【追加】編集状態の変化を監視して、表示画面へ通知する
watch(isEditing, (newVal) => {
    emit('edit-mode-update', newVal);
});

// 編集開始
const startEdit = () => {
    tempGrid.value = { ...grid.value };
    isEditing.value = true;
    emit('edit-mode-update', true);
};
// 確定（反映・保存）
const confirmEdit = async () => {
    grid.value = { ...tempGrid.value };
    // 表示画面へ同期
    emit('grid-update', grid.value);
    // Rust経由でファイル保存
    try {
        await invoke('save_settings', { config: grid.value });
    } catch (e) { console.error(e); }
    isEditing.value = false;
    emit('edit-mode-update', false);
    // alert("設定を確定・保存しました。");
};

// キャンセル（元の値に戻す）
const cancelEdit = () => {
    tempGrid.value = { ...grid.value };
    isEditing.value = false;
    emit('grid-update', { ...grid.value }); // 表示を元に戻す
};

const spin = () => {
    // アニメーション中の連続クリックをガード
    if (isAnimating.value) return;

    const available = Array.from({ length: 25 }, (_, i) => i + 1)
        .filter(n => !hitHistory.value.includes(n));
    if (available.length === 0) return alert("全て当選済みです");

    const num = available[Math.floor(Math.random() * available.length)];
    isAnimating.value = true; // ガード開始

    emit('bingo-hit', { number: num });
};

const resetBingo = () => {
    if (confirm("履歴をリセットしますか？")) {
        hitHistory.value = [];
        emit('bingo-reset', {});
    }
};

</script>

<template>
    <div class="panel">
        <h3>🎡 Bingo Operation</h3>

        <section class="spin-section">
            <button class="spin-btn" :class="{ 'is-animating': isAnimating }" :disabled="isAnimating" @click="spin">
                {{ isAnimating ? '抽選中...' : 'SPIN BINGO' }}
            </button>
            <button class="reset-btn" @click="resetBingo">RESET</button>
        </section>

        <section class="history-section">
            <h4>当選履歴 ({{ hitHistory.length }} / 25)</h4>
            <div class="history-list">
                <span v-for="num in hitHistory" :key="num" class="history-tag">{{ num }}</span>
            </div>
        </section>
        <hr />
        <section class="adjust-section">
            <div class="toggle-header" @click="isToggleOpen = !isToggleOpen">
                <h3>⚙️ 各種設定 (グリッド・音響)</h3>
                <span>{{ isToggleOpen ? '▲ 閉じる' : '▼ 開く' }}</span>
            </div>

            <div v-if="isToggleOpen" class="toggle-content">
                <div class="audio-settings">
                    <div class="setting-group">
                        <h4>🔊 効果音 (SE)</h4>
                        <label><input type="checkbox" v-model="tempGrid.se_enabled"> 有効</label>
                        <input type="range" min="0" max="100" v-model.number="tempGrid.se_volume"
                            :disabled="!tempGrid.se_enabled">
                        <span class="value-display">音量: {{ tempGrid.se_volume }}%</span>
                    </div>

                    <div class="setting-group">
                        <h4>🗣️ 読み上げ (TTS)</h4>
                        <label><input type="checkbox" v-model="tempGrid.tts_enabled"> 有効</label>
                        <input type="range" min="0" max="100" v-model.number="tempGrid.tts_volume"
                            :disabled="!tempGrid.tts_enabled">
                        <span class="value-display">音量: {{ tempGrid.tts_volume }}%</span>
                        <div class="repeat-select" v-if="tempGrid.tts_enabled">
                            <span>読み上げ回数: </span>
                            <select v-model.number="tempGrid.tts_repeat_count">
                                <option v-for="i in 3" :key="i" :value="i">{{ i }}回</option>
                            </select>
                        </div>
                    </div>
                </div>

                <hr style="opacity: 0.1; margin: 15px 0;" />

                <button v-if="!isEditing" class="edit-btn" @click="startEdit">グリッド位置を調整する</button>

                <div v-if="isEditing" class="editing-controls">
                    <div class="edit-actions">
                        <button class="confirm-btn" @click="confirmEdit">設定を保存して終了</button>
                        <button class="cancel-btn" @click="cancelEdit">キャンセル</button>
                    </div>
                    <div class="sliders">
                        <label>X: <input type="range" min="0" max="282" v-model.number="tempGrid.x" /><span
                                class="val">{{ tempGrid.x }}px</span></label>
                        <label>Y: <input type="range" min="0" max="368" v-model.number="tempGrid.y" /><span
                                class="val">{{ tempGrid.y }}px</span></label>
                        <label>W: <input type="range" min="0" max="282" v-model.number="tempGrid.w" /><span
                                class="val">{{ tempGrid.w }}px</span></label>
                        <label>H: <input type="range" min="0" max="368" v-model.number="tempGrid.h" /><span
                                class="val">{{ tempGrid.h }}px</span></label>

                        <label class="hit-scale-label">
                            🎯 スタンプ縮尺: <input type="range" min="10" max="200" v-model.number="tempGrid.hit_scale" />
                            <span class="val">{{ tempGrid.hit_scale }}%</span>
                        </label>
                    </div>
                </div>
            </div>
        </section>
    </div>
</template>

<style scoped>
.panel {
    padding: 20px;
    background: #2c3e50;
    color: white;
    height: 100vh;
    overflow-y: auto;
}

.setting-group {
    margin-bottom: 15px;
}

.setting-group h4 {
    margin: 5px 0;
    font-size: 0.9rem;
    color: #3498db;
}

.repeat-select {
    margin-top: 5px;
    font-size: 0.8rem;
}

.repeat-select select {
    background: #34495e;
    color: white;
    border: 1px solid rgba(255, 255, 255, 0.2);
    border-radius: 3px;
}

.spin-btn {
    width: 100%;
    height: 60px;
    padding: 15px;
    font-size: 1.2rem;
    background: #e74c3c;
    border: none;
    color: white;
    cursor: pointer;
    border-radius: 4px;
}

.reset-btn {
    width: 100%;
    margin-top: 5px;
    padding: 5px;
    background: #7f8c8d;
    border: none;
    color: white;
    cursor: pointer;
    border-radius: 4px;
}

.history-section {
    margin: 15px 0;
    background: rgba(0, 0, 0, 0.2);
    padding: 10px;
    border-radius: 4px;
}

.history-list {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-top: 5px;
}

.history-tag {
    background: #f1c40f;
    color: #333;
    padding: 2px 8px;
    border-radius: 10px;
    font-weight: bold;
    font-size: 0.9rem;
}

.adjust-section {
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 4px;
    margin-top: 20px;
}

.toggle-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px;
    cursor: pointer;
    background: rgba(255, 255, 255, 0.05);
}

.toggle-header h3 {
    margin: 0;
    font-size: 1rem;
}

.toggle-content {
    padding: 15px;
    background: rgba(0, 0, 0, 0.1);
}

.edit-btn {
    width: 100%;
    padding: 8px;
    background: #3498db;
    color: white;
    border: none;
    cursor: pointer;
    margin-bottom: 15px;
}

.is-locked {
    opacity: 0.5;
    pointer-events: none;
}

.sliders label {
    display: block;
    margin: 10px 0;
    font-size: 0.9rem;
}

input[type="range"] {
    width: 100%;
}

.edit-actions {
    display: flex;
    gap: 10px;
    margin-top: 20px;
}

.confirm-btn {
    flex: 2;
    padding: 10px;
    background: #27ae60;
    border: none;
    color: white;
    cursor: pointer;
}

.cancel-btn {
    flex: 1;
    padding: 10px;
    background: #95a5a6;
    border: none;
    color: white;
    cursor: pointer;
}
</style>