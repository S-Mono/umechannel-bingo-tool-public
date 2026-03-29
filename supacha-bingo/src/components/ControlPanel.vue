<script setup lang="ts">
import { ref, watch, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';

// 確定済みの位置情報（表示画面と同期している値）
const grid = ref({ x: 22, y: 103, w: 237, h: 239, hit_scale: 100 }); // hit_scale追加

// 編集中のアコーディオン（トグル）の開閉状態
const isToggleOpen = ref(false);

// 編集モード（変更ボタン押下後）の状態
const isEditing = ref(false);

// 編集中の値を一時的に保持する変数
const tempGrid = ref({ ...grid.value });

// 当選履歴
const hitHistory = ref<number[]>([]);

onMounted(async () => {
    try {
        const saved = await invoke<any>('load_settings');
        grid.value = saved;
        tempGrid.value = { ...saved };
        // 起動時に現在の値を表示側に同期
        emit('grid-update', grid.value);
    } catch (e) { console.error(e); }
});

// 【追加】編集状態の変化を監視して、表示画面へ通知する
watch(isEditing, (newVal) => {
    emit('edit-mode-update', newVal);
});

// 編集開始
const startEdit = () => {
    tempGrid.value = { ...grid.value };
    isEditing.value = true;
};

// 確定（反映・保存）
const confirmEdit = async () => {
    grid.value = { ...tempGrid.value };
    // 表示画面へ同期
    emit('grid-update', grid.value);
    // Rust経由でファイル保存
    await invoke('save_settings', { config: grid.value });
    isEditing.value = false;
    alert("設定を確定・保存しました。");
};

// キャンセル（元の値に戻す）
const cancelEdit = () => {
    tempGrid.value = { ...grid.value };
    isEditing.value = false;
};

const spin = () => {
    const available = Array.from({ length: 25 }, (_, i) => i + 1).filter(n => !hitHistory.value.includes(n));
    if (available.length === 0) return alert("全て当選済みです");

    const num = available[Math.floor(Math.random() * available.length)];
    hitHistory.value.push(num);
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
            <button class="spin-btn" @click="spin">SPIN BINGO</button>
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
                <h3>📏 グリッド微調整 (px)</h3>
                <span>{{ isToggleOpen ? '▲ 閉じる' : '▼ 開く' }}</span>
            </div>

            <div v-if="isToggleOpen" class="toggle-content">
                <button v-if="!isEditing" class="edit-btn" @click="startEdit">変更を開始する</button>

                <div class="sliders" :class="{ 'is-locked': !isEditing }">
                    <label>X: {{ tempGrid.x }}px <input type="range" min="0" max="282" v-model.number="tempGrid.x"
                            :disabled="!isEditing" /></label>
                    <label>Y: {{ tempGrid.y }}px <input type="range" min="0" max="368" v-model.number="tempGrid.y"
                            :disabled="!isEditing" /></label>
                    <label>W: {{ tempGrid.w }}px <input type="range" min="0" max="282" v-model.number="tempGrid.w"
                            :disabled="!isEditing" /></label>
                    <label>H: {{ tempGrid.h }}px <input type="range" min="0" max="368" v-model.number="tempGrid.h"
                            :disabled="!isEditing" /></label>

                    <label style="color: #f1c40f; font-weight: bold;">
                        🎯 スタンプ縮尺: {{ tempGrid.hit_scale }}%
                        <input type="range" min="10" max="200" v-model.number="tempGrid.hit_scale"
                            :disabled="!isEditing" />
                    </label>
                </div>

                <div v-if="isEditing" class="edit-actions">
                    <button class="confirm-btn" @click="confirmEdit">確定 (保存・反映)</button>
                    <button class="cancel-btn" @click="cancelEdit">キャンセル</button>
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

.spin-btn {
    width: 100%;
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