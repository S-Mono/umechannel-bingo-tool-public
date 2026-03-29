<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { emit, listen } from '@tauri-apps/api/event';
import confetti from 'canvas-confetti';

const BG_IMAGE_PATH = '/assets/background.png';
const HIT_MARK_IMAGE_PATH = '/assets/hit_mark.png';
const SE_SPIN_PATH = '/assets/audio/spin_loop.mp3';
const SE_WIN_PATH = '/assets/audio/win_confirm.mp3';

const gridPos = ref({
    x: 22, y: 109, w: 237, h: 239, hit_scale: 100,
    se_enabled: true, se_volume: 20, tts_enabled: true, tts_volume: 40, tts_repeat_count: 1
});
const hitNumbers = ref<number[]>([]);
const isSpinning = ref(false);
const rouletteNumber = ref<number | null>(null);
const showBorder = ref(false);
let spinAudio: HTMLAudioElement | null = null;

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
        if (!spinAudio) spinAudio = new Audio(SE_SPIN_PATH);
        spinAudio.loop = true;
        spinAudio.volume = gridPos.value.se_volume / 100;
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

onMounted(async () => {
    await listen<any>('grid-update', (e) => { gridPos.value = e.payload; });
    await listen<boolean>('edit-mode-update', (e) => { showBorder.value = e.payload; });
    await listen<{ number: number }>('bingo-hit', (e) => { startRoulette(e.payload.number); });
    await listen('bingo-reset', () => { hitNumbers.value = []; rouletteNumber.value = null; });
    await listen<{ hits: number[] }>('bingo-sync-hits', (e) => {
        hitNumbers.value = e.payload.hits;
        rouletteNumber.value = null;
    });
});
</script>

<template>
    <div class="bingo-view-container" data-tauri-drag-region>
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
                    <img v-if="hitNumbers.includes(n)" :src="HIT_MARK_IMAGE_PATH" class="stamp"
                        :style="{ transform: `scale(${gridPos.hit_scale / 100})` }" />
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
}

.card-bg-img {
    position: absolute;
    width: 100%;
    height: 100%;
    object-fit: contain;
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
}

.roulette-number {
    font-size: 2.5rem;
    color: #f1c40f;
    font-weight: bold;
    text-shadow: 2px 2px 0 #000;
}

.grid-layer {
    position: absolute;
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    grid-template-rows: repeat(5, 1fr);
    pointer-events: none;
}

.cell {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid transparent;
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
    width: 80%;
    height: 80%;
    object-fit: contain;
    margin-top: -10px;
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