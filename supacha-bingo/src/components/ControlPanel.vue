<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import BingoModal from './BingoModal.vue';
import { getCompletedBingoKeys, getNewBingoKeys } from '../utils/bingo';

const modal = ref<InstanceType<typeof BingoModal> | null>(null);

/** --- 1. 型定義と初期状態 --- */
interface GridConfig {
    x: number; y: number; w: number; h: number; hit_scale: number;
    se_enabled: boolean; se_volume: number;
    tts_enabled: boolean; tts_volume: number; tts_repeat_count: number;
    effect_enabled: boolean;
    effect_monitor_id: string;
    normal_bingo_effect_enabled: boolean;
    special_1_effect_enabled: boolean;
    special_25_effect_enabled: boolean;
    normal_bingo_video_path: string;
    special_1_video_path: string;
    special_25_video_path: string;
}

interface EffectMonitor {
    id: string;
    label: string;
}

type EffectType = 'NORMAL_BINGO' | 'SPECIAL_1' | 'SPECIAL_25';

interface EffectPlaybackStartedPayload {
    effectType: EffectType;
}

interface EffectPlaybackStoppedPayload {
    effectType: EffectType | null;
}

const grid = ref<GridConfig>({
    x: 22, y: 109, w: 237, h: 239, hit_scale: 100,
    se_enabled: true, se_volume: 50, tts_enabled: true, tts_volume: 50, tts_repeat_count: 1,
    effect_enabled: true,
    effect_monitor_id: '',
    normal_bingo_effect_enabled: true,
    special_1_effect_enabled: false,
    special_25_effect_enabled: false,
    normal_bingo_video_path: 'effects/normal_bingo.mp4',
    special_1_video_path: 'effects/special_1.mp4',
    special_25_video_path: 'effects/special_25.mp4'
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
const effectMonitors = ref<EffectMonitor[]>([]);
const bingoLineKeys = ref<string[]>([]);
const previewingEffectType = ref<EffectType | null>(null);
const isEffectPlaying = ref(false);
let settingsSaveTimer: ReturnType<typeof window.setTimeout> | null = null;
let unlistenAnimationFinished: (() => void) | null = null;
let unlistenEffectPlaybackStarted: (() => void) | null = null;
let unlistenEffectPlaybackStopped: (() => void) | null = null;

const cloneGridConfig = (config: GridConfig): GridConfig => ({ ...config });

const getCurrentConfig = (): GridConfig => cloneGridConfig(tempGrid.value);

const commitCurrentConfig = (): GridConfig => {
    const nextConfig = getCurrentConfig();
    grid.value = nextConfig;
    return nextConfig;
};

const saveCurrentSettings = async () => {
    const nextConfig = commitCurrentConfig();
    await invoke('save_settings', { config: nextConfig });
};

/** --- 2. ライフサイクル --- */
onMounted(async () => {
    try {
        const saved = await invoke<any>('load_settings');
        grid.value = { ...grid.value, ...saved };
        tempGrid.value = cloneGridConfig(grid.value);
        emit('grid-update', cloneGridConfig(grid.value));
        await refreshSessionList();
        await refreshEffectMonitors();
        await syncEffectWindow(false, grid.value);

        unlistenEffectPlaybackStarted = await listen<EffectPlaybackStartedPayload>('effect-playback-started', () => {
            isEffectPlaying.value = true;
        });

        unlistenEffectPlaybackStopped = await listen<EffectPlaybackStoppedPayload>('effect-playback-stopped', (event) => {
            isEffectPlaying.value = false;
            if (!event.payload.effectType || previewingEffectType.value === event.payload.effectType) {
                previewingEffectType.value = null;
            }
        });

        unlistenAnimationFinished = await listen<{ number: number }>('bingo-animation-finished', async (event) => {
            if (!isLive.value) return;

            if (!hitHistory.value.includes(event.payload.number)) {
                hitHistory.value = [...hitHistory.value, event.payload.number];
            }

            const newBingoKeys = getNewBingoKeys(bingoLineKeys.value, hitHistory.value);
            bingoLineKeys.value = getCompletedBingoKeys(hitHistory.value);
            isAnimating.value = false;

            if (newBingoKeys.length > 0) {
                await playBingoEffect(event.payload.number);
            }

            await persistHits('HIT');
        });
    } catch (e) { console.error("Initialize Error:", e); }
});

onUnmounted(() => {
    if (settingsSaveTimer) {
        window.clearTimeout(settingsSaveTimer);
        settingsSaveTimer = null;
    }

    if (unlistenAnimationFinished) {
        unlistenAnimationFinished();
        unlistenAnimationFinished = null;
    }

    if (unlistenEffectPlaybackStarted) {
        unlistenEffectPlaybackStarted();
        unlistenEffectPlaybackStarted = null;
    }

    if (unlistenEffectPlaybackStopped) {
        unlistenEffectPlaybackStopped();
        unlistenEffectPlaybackStopped = null;
    }
});

const refreshSessionList = async () => {
    sessionFiles.value = await invoke<string[]>('get_sessions');
};

const refreshEffectMonitors = async () => {
    effectMonitors.value = await invoke<EffectMonitor[]>('list_effect_monitors');
};

const syncEffectWindow = async (visible = false, config: GridConfig = getCurrentConfig()) => {
    try {
        await invoke('sync_effect_window', { config, visible });
    } catch (error) {
        console.error('Effect Window Sync Error:', error);
    }
};

const setEffectMonitorHighlight = async (active: boolean) => {
    if (isEffectPlaying.value) {
        return;
    }

    await syncEffectWindow(active);
    emit('effect-monitor-highlight', { active });
};

const syncBingoLines = (hits: number[]) => {
    bingoLineKeys.value = getCompletedBingoKeys(hits);
};

const scheduleSettingsSave = () => {
    if (isEditing.value) return;

    if (settingsSaveTimer) {
        window.clearTimeout(settingsSaveTimer);
    }

    settingsSaveTimer = window.setTimeout(async () => {
        try {
            await saveCurrentSettings();
        } catch (error) {
            console.error('Settings Auto Save Error:', error);
        } finally {
            settingsSaveTimer = null;
        }
    }, 250);
};

const playBingoEffect = async (lastNumber: number) => {
    const currentConfig = getCurrentConfig();
    if (!currentConfig.effect_enabled) return;

    let effectType: EffectType = 'NORMAL_BINGO';
    if (lastNumber === 1 && currentConfig.special_1_effect_enabled) {
        effectType = 'SPECIAL_1';
    } else if (lastNumber === 25 && currentConfig.special_25_effect_enabled) {
        effectType = 'SPECIAL_25';
    } else if (!currentConfig.normal_bingo_effect_enabled) {
        return;
    }

    try {
        await invoke('play_bingo_effect', { config: currentConfig, effectType });
    } catch (error) {
        console.error('Effect Playback Error:', error);
    }
};

const previewEffect = async (effectType: EffectType) => {
    previewingEffectType.value = effectType;

    try {
        const played = await invoke<boolean>('preview_bingo_effect', { config: getCurrentConfig(), effectType });
        if (!played) {
            previewingEffectType.value = null;
            await modal.value?.show('プレビューする動画が見つかりませんでした。\n動画の場所を確認してください。', 'alert');
        }
    } catch (error) {
        previewingEffectType.value = null;
        console.error('Effect Preview Error:', error);
        await modal.value?.show('プレビュー再生に失敗しました。\n設定した画面や動画の場所を確認してください。', 'alert');
    }
};

const stopPreviewEffect = async () => {
    try {
        await emit('stop-effect-playback', {});
    } catch (error) {
        console.error('Effect Preview Stop Error:', error);
    }
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
    bingoLineKeys.value = [];
    emit('bingo-reset', {});
};

const previewSession = async (filename: string) => {
    if (!filename) {
        currentFile.value = null;
        hitHistory.value = [];
        redoStack.value = [];
        isLive.value = false;
        bingoLineKeys.value = [];
        emit('bingo-reset', {});
        await invoke('log_action', { trigger: 'CLOSE', message: 'プレビュー終了' });
        return;
    }
    const hits = await invoke<number[]>('load_session', { filename });
    hitHistory.value = hits;
    syncBingoLines(hits);
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

const isSpinning = ref(false); // ルーレット回転中フラグ
const spin = () => {
    // 閲覧モード中、または演出中（かつ回転中でない）は操作不能
    if (!isLive.value || (isAnimating.value && !isSpinning.value)) return;

    if (!isSpinning.value) {
        // --- [フェーズ1] 抽選開始（回転アニメーション スタート） ---
        isSpinning.value = true;
        // BingoCardに「回転開始」を通知し、確定させる番号を予約する
        emit('bingo-spin-start', {});

    } else {
        // --- [フェーズ2] ストップ押下（回転アニメーション ストップ＆番号確定） ---
        const available = Array.from({ length: 25 }, (_, i) => i + 1).filter(n => !hitHistory.value.includes(n));
        if (available.length === 0) return modal.value?.show("番号はすべて選出しました！", "alert");

        redoStack.value = [];
        // この時点で裏側では番号を確定させる（ガチ選出）
        const randomIndex = getSecureRandomInt(available.length);
        const num = available[randomIndex];

        isSpinning.value = false;
        isAnimating.value = true; // 確定演出が終わるまでボタンをロック
        // BingoCardに「回転を止めて確定させろ」と通知
        emit('bingo-spin-stop', { number: num });
    }
};

const undo = async () => {
    if (!isLive.value || hitHistory.value.length === 0) return;
    const last = hitHistory.value.pop();
    if (last) redoStack.value.push(last);
    syncBingoLines(hitHistory.value);
    emit('bingo-sync-hits', { hits: [...hitHistory.value] });
    await persistHits('UNDO');
};

const redo = async () => {
    if (!isLive.value || redoStack.value.length === 0) return;
    const last = redoStack.value.pop();
    if (last) hitHistory.value.push(last);
    syncBingoLines(hitHistory.value);
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
        bingoLineKeys.value = [];
        emit('bingo-reset', {});
        await invoke('log_action', { trigger: 'RESET', message: 'セッション初期化' });
    }
};

/** --- 5. 設定管理 --- */
watch(tempGrid, (val) => {
    emit('grid-update', { ...val });
    scheduleSettingsSave();
}, { deep: true });
watch(() => [tempGrid.value.effect_enabled, tempGrid.value.effect_monitor_id], () => {
    syncEffectWindow(false);
});
watch(isEditing, (val) => { emit('edit-mode-update', val); });

const startEdit = () => { isEditing.value = true; };
const confirmEdit = async () => {
    if (settingsSaveTimer) {
        window.clearTimeout(settingsSaveTimer);
        settingsSaveTimer = null;
    }

    await saveCurrentSettings();
    isEditing.value = false;
};
const cancelEdit = () => {
    if (settingsSaveTimer) {
        window.clearTimeout(settingsSaveTimer);
        settingsSaveTimer = null;
    }

    tempGrid.value = cloneGridConfig(grid.value);
    isEditing.value = false;
    emit('grid-update', cloneGridConfig(grid.value));
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
            <button class="spin-btn" :disabled="!isLive || (isAnimating && !isSpinning)" @click="spin">
                <template v-if="!isLive">⚠️ 閲覧モード</template>
                <template v-else>
                    {{ isSpinning ? '！！ ストップ ！！' : (isAnimating ? '確定演出中...' : '！抽選開始！') }}
                </template>
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
                <span>⚙️ 設定 (位置・音響・演出)</span>
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

                <div class="effect-group">
                    <div class="setting-item">
                        <div class="setting-header">
                            <label class="item-label">エフェクト再生</label>
                            <button class="toggle-btn" :class="{ 'is-active': tempGrid.effect_enabled }"
                                @click="tempGrid.effect_enabled = !tempGrid.effect_enabled">
                                {{ tempGrid.effect_enabled ? 'ON' : 'OFF' }}
                            </button>
                        </div>
                    </div>

                    <div class="setting-item"
                        @mouseenter="setEffectMonitorHighlight(true)"
                        @mouseleave="setEffectMonitorHighlight(false)">
                        <label class="item-label">再生モニタ</label>
                        <select v-model="tempGrid.effect_monitor_id" class="select-input"
                            :disabled="!tempGrid.effect_enabled || isEffectPlaying">
                            <option value="">プライマリモニタ（自動）</option>
                            <option v-for="monitor in effectMonitors" :key="monitor.id" :value="monitor.id">
                                {{ monitor.label }}
                            </option>
                        </select>
                    </div>

                    <div class="setting-item">
                        <div class="setting-header">
                            <label class="item-label">ビンゴ動画</label>
                            <button class="toggle-btn" :class="{ 'is-active': tempGrid.normal_bingo_effect_enabled }"
                                @click="tempGrid.normal_bingo_effect_enabled = !tempGrid.normal_bingo_effect_enabled"
                                :disabled="!tempGrid.effect_enabled">
                                {{ tempGrid.normal_bingo_effect_enabled ? 'ON' : 'OFF' }}
                            </button>
                        </div>
                        <div class="path-row">
                            <input v-model="tempGrid.normal_bingo_video_path" class="path-input"
                                :disabled="!tempGrid.effect_enabled || !tempGrid.normal_bingo_effect_enabled"
                                placeholder="effects/normal_bingo.mp4">
                            <button class="preview-btn"
                                :disabled="previewingEffectType !== null && previewingEffectType !== 'NORMAL_BINGO'"
                                @click="previewingEffectType === 'NORMAL_BINGO' ? stopPreviewEffect() : previewEffect('NORMAL_BINGO')">
                                {{ previewingEffectType === 'NORMAL_BINGO' ? '停止' : 'プレビュー' }}
                            </button>
                        </div>
                    </div>

                    <div class="setting-item">
                        <div class="setting-header">
                            <label class="item-label">1 用動画</label>
                            <button class="toggle-btn" :class="{ 'is-active': tempGrid.special_1_effect_enabled }"
                                @click="tempGrid.special_1_effect_enabled = !tempGrid.special_1_effect_enabled"
                                :disabled="!tempGrid.effect_enabled">
                                {{ tempGrid.special_1_effect_enabled ? 'ON' : 'OFF' }}
                            </button>
                        </div>
                        <div class="path-row">
                            <input v-model="tempGrid.special_1_video_path" class="path-input"
                                :disabled="!tempGrid.effect_enabled || !tempGrid.special_1_effect_enabled"
                                placeholder="effects/special_1.mp4">
                            <button class="preview-btn"
                                :disabled="previewingEffectType !== null && previewingEffectType !== 'SPECIAL_1'"
                                @click="previewingEffectType === 'SPECIAL_1' ? stopPreviewEffect() : previewEffect('SPECIAL_1')">
                                {{ previewingEffectType === 'SPECIAL_1' ? '停止' : 'プレビュー' }}
                            </button>
                        </div>
                    </div>

                    <div class="setting-item">
                        <div class="setting-header">
                            <label class="item-label">25 用動画</label>
                            <button class="toggle-btn" :class="{ 'is-active': tempGrid.special_25_effect_enabled }"
                                @click="tempGrid.special_25_effect_enabled = !tempGrid.special_25_effect_enabled"
                                :disabled="!tempGrid.effect_enabled">
                                {{ tempGrid.special_25_effect_enabled ? 'ON' : 'OFF' }}
                            </button>
                        </div>
                        <div class="path-row">
                            <input v-model="tempGrid.special_25_video_path" class="path-input"
                                :disabled="!tempGrid.effect_enabled || !tempGrid.special_25_effect_enabled"
                                placeholder="effects/special_25.mp4">
                            <button class="preview-btn"
                                :disabled="previewingEffectType !== null && previewingEffectType !== 'SPECIAL_25'"
                                @click="previewingEffectType === 'SPECIAL_25' ? stopPreviewEffect() : previewEffect('SPECIAL_25')">
                                {{ previewingEffectType === 'SPECIAL_25' ? '停止' : 'プレビュー' }}
                            </button>
                        </div>
                    </div>

                    <p class="setting-note">動画は 2 通りで指定できます。アプリ本体の近くに置く書き方か、PC 上の場所をそのまま書く方法です。</p>
                    <p class="setting-note">例: effects/normal_bingo.mp4</p>
                    <p class="setting-note">例: D:/StreamAssets/bingo/normal_bingo.mp4</p>
                    <p class="setting-note">effects/ から始まる書き方は、spacha-bingo.exe と同じ場所を起点に探します。</p>
                    <p class="setting-note">1 用や 25 用の動画が見つからないときは通常のビンゴ動画を使い、それも無いときは動画を流しません。</p>
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

.effect-group {
    display: flex;
    flex-direction: column;
    gap: 2px;
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

.select-input,
.path-input {
    width: 100%;
    box-sizing: border-box;
    background: #2c3e50;
    color: white;
    border: 1px solid #444;
    border-radius: 4px;
    padding: 8px 10px;
    font-size: 0.85rem;
}

.select-input:disabled,
.path-input:disabled {
    opacity: 0.4;
    cursor: not-allowed;
}

.path-row {
    display: flex;
    gap: 8px;
}

.preview-btn {
    flex: 0 0 auto;
    min-width: 88px;
    padding: 8px 12px;
    background: #16a085;
    color: white;
    border: 1px solid #1abc9c;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.8rem;
    font-weight: bold;
}

.preview-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    filter: grayscale(1);
}

.setting-note {
    margin: -4px 0 6px;
    color: #95a5a6;
    font-size: 0.75rem;
    line-height: 1.4;
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