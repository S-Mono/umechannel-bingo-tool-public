<script setup lang="ts">
import { ref } from 'vue';

// 内部状態
const isOpen = ref(false);
const message = ref('');
const isConfirm = ref(false);
let resolvePromise: (value: boolean) => void;

// 外部から呼び出す関数
const show = (msg: string, type: 'alert' | 'confirm' = 'alert'): Promise<boolean> => {
    message.value = msg;
    isConfirm.value = type === 'confirm';
    isOpen.value = true;

    return new Promise((resolve) => {
        resolvePromise = resolve;
    });
};

const handleAction = (result: boolean) => {
    isOpen.value = false;
    if (resolvePromise) resolvePromise(result);
};

// 親コンポーネントからこの関数を使えるように公開
defineExpose({ show });
</script>

<template>
    <Teleport to="body">
        <Transition name="modal-fade">
            <div v-if="isOpen" class="modal-overlay" @click.self="handleAction(false)">
                <div class="modal-content">
                    <div class="modal-header">
                        <span class="warning-icon">⚠️</span>
                        <span class="header-text">SYSTEM MESSAGE</span>
                    </div>

                    <div class="modal-body">
                        {{ message }}
                    </div>

                    <div class="modal-footer">
                        <button v-if="isConfirm" class="btn-cancel" @click="handleAction(false)">CANCEL</button>
                        <button class="btn-ok" @click="handleAction(true)">OK</button>
                    </div>
                </div>
            </div>
        </Transition>
    </Teleport>
</template>

<style scoped>
.modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
}

.modal-content {
    background: #1a2a3a;
    border: 1px solid #f39c12;
    /* ゴールドのアクセント */
    width: 320px;
    padding: 0;
    border-radius: 4px;
    box-shadow: 0 0 20px rgba(0, 0, 0, 0.5);
}

.modal-header {
    background: rgba(243, 156, 18, 0.1);
    padding: 10px 15px;
    border-bottom: 1px solid rgba(243, 156, 18, 0.3);
    display: flex;
    align-items: center;
    gap: 10px;
}

.header-text {
    font-size: 0.8rem;
    font-weight: bold;
    color: #f39c12;
    letter-spacing: 1px;
}

.modal-body {
    padding: 25px 20px;
    color: #eee;
    font-size: 0.95rem;
    line-height: 1.5;
    text-align: center;
    white-space: pre-wrap;
}

.modal-footer {
    padding: 15px;
    display: flex;
    gap: 10px;
    justify-content: flex-end;
    background: rgba(0, 0, 0, 0.2);
}

button {
    padding: 8px 20px;
    border: none;
    border-radius: 2px;
    cursor: pointer;
    font-weight: bold;
    font-size: 0.85rem;
    transition: 0.2s;
}

.btn-ok {
    background: #27ae60;
    color: white;
}

.btn-ok:hover {
    background: #2ecc71;
}

.btn-cancel {
    background: #7f8c8d;
    color: white;
}

.btn-cancel:hover {
    background: #95a5a6;
}

/* アニメーション */
.modal-fade-enter-active,
.modal-fade-leave-active {
    transition: opacity 0.3s;
}

.modal-fade-enter-from,
.modal-fade-leave-to {
    opacity: 0;
}
</style>