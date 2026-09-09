<template>
  <ModalDialog :title="$t('ocr.title')" :width="480" @cancel="close">
    <!-- unavailable -->
    <section v-if="unavailable" class="space-y-2">
      <p class="text-sm">{{ unavailableText }}</p>
      <p class="text-xs text-base-content/50">{{ $t('ocr.unavailable_hint') }}</p>
    </section>

    <!-- ready to start -->
    <section v-else-if="!running && !completed" class="space-y-3">
      <p class="text-sm">{{ $t('ocr.summary', { count: files.length }) }}</p>
      <p class="text-xs text-base-content/50 leading-relaxed">{{ $t('ocr.explain') }}</p>
      <div class="flex items-center justify-between p-1 rounded-box hover:bg-base-100/10">
        <div class="flex flex-col gap-0.5 text-sm leading-5">
          <div>{{ $t('ocr.redo') }}</div>
          <div class="text-xs text-base-content/50">{{ $t('ocr.redo_hint') }}</div>
        </div>
        <input v-model="redo" type="checkbox" class="toggle toggle-primary toggle-sm shrink-0" />
      </div>
    </section>

    <!-- progress -->
    <section v-else class="space-y-3">
      <p class="text-sm">{{ completed ? completionText : $t('ocr.working') }}</p>
      <progress
        class="progress progress-primary w-full"
        :value="progress.processed"
        :max="Math.max(progress.total, 1)"
      ></progress>
      <p class="text-xs tabular-nums text-base-content/50">
        {{ $t('ocr.counts', {
          processed: progress.processed,
          total: progress.total,
          withText: progress.withText,
          failed: progress.failed,
        }) }}
      </p>
      <ul v-if="errors.length" class="max-h-24 overflow-y-auto text-xs text-error space-y-1">
        <li v-for="(message, index) in errors" :key="index" class="truncate" :title="message">{{ message }}</li>
      </ul>
    </section>

    <div class="mt-5 flex justify-end gap-3">
      <button v-if="!completed && !unavailable" class="t-button-default" :disabled="cancelling" @click="close">
        {{ running ? $t('ocr.stop') : $t('msgbox.cancel') }}
      </button>
      <button v-if="completed || unavailable" class="t-button-primary" @click="$emit('cancel')">
        {{ $t('msgbox.ok') }}
      </button>
      <button v-else class="t-button-primary" :disabled="running || files.length === 0" @click="start">
        {{ $t('ocr.start') }}
      </button>
    </div>
  </ModalDialog>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import ModalDialog from '@/components/ModalDialog.vue';
import { useToast } from '@/common/toast';
import { useUIStore } from '@/stores/uiStore';
import {
  cancelRecognizeText,
  getOcrStatus,
  listenOcrFinished,
  listenOcrProgress,
  recognizeText,
} from '@/common/api';

const props = defineProps<{ files: any[] }>();
const emit = defineEmits(['cancel', 'complete']);

const { t } = useI18n();
const toast = useToast();
const uiStore = useUIStore();

const availability = ref<string>('ready');
const redo = ref(false);
const running = ref(false);
const cancelling = ref(false);
const completed = ref(false);
const errors = ref<string[]>([]);
const progress = ref({
  processed: 0,
  total: props.files.length,
  withText: 0,
  failed: 0,
});

let unlistenProgress: (() => void) | undefined;
let unlistenFinished: (() => void) | undefined;

const unavailable = computed(() => availability.value !== 'ready');

const unavailableText = computed(() =>
  availability.value === 'unsupported_platform'
    ? t('ocr.unavailable_platform')
    : t('ocr.unavailable_language'),
);

const completionText = computed(() => {
  if (progress.value.cancelled) return t('ocr.done_cancelled', { processed: progress.value.processed });
  if (progress.value.failed > 0) {
    return t('ocr.done_with_failures', {
      withText: progress.value.withText,
      failed: progress.value.failed,
    });
  }
  return t('ocr.done', { withText: progress.value.withText, processed: progress.value.processed });
});

async function start() {
  running.value = true;
  completed.value = false;
  errors.value = [];
  progress.value = { processed: 0, total: props.files.length, withText: 0, failed: 0 };

  unlistenProgress = await listenOcrProgress((event: any) => {
    progress.value = { ...progress.value, ...(event.payload || {}) };
  });

  try {
    let resolveFinished: (value: any) => void = () => {};
    let rejectFinished: (reason?: any) => void = () => {};
    const finished = new Promise<any>((resolve, reject) => {
      resolveFinished = resolve;
      rejectFinished = reject;
    });
    unlistenFinished = await listenOcrFinished((event: any) => {
      const payload = event.payload || {};
      if (payload.error) rejectFinished(new Error(payload.error));
      else resolveFinished(payload.result);
    });

    await recognizeText(
      props.files.map((file) => Number(file.id)).filter(Boolean),
      redo.value,
    );

    const result = await finished;
    progress.value = { ...progress.value, ...result };
    errors.value = Array.isArray(result?.errors) ? result.errors : [];
    completed.value = true;
    emit('complete', result);
  } catch (error) {
    toast.error(String(error));
    completed.value = true;
  } finally {
    running.value = false;
    unlistenProgress?.();
    unlistenProgress = undefined;
    unlistenFinished?.();
    unlistenFinished = undefined;
  }
}

async function close() {
  if (running.value) {
    if (cancelling.value) return;
    cancelling.value = true;
    try {
      await cancelRecognizeText();
    } finally {
      cancelling.value = false;
    }
    return;
  }
  emit('cancel');
}

onMounted(async () => {
  uiStore.pushInputHandler('OcrDialog');
  const status = await getOcrStatus();
  if (status?.availability) availability.value = String(status.availability);
});

onBeforeUnmount(() => {
  uiStore.removeInputHandler('OcrDialog');
  unlistenProgress?.();
  unlistenFinished?.();
});
</script>
