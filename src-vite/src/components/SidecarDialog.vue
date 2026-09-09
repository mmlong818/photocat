<template>
  <ModalDialog :title="$t('sidecar.title')" :width="520" @cancel="close">
    <!-- choose a direction -->
    <section v-if="!running && !completed" class="space-y-4">
      <p class="text-sm text-base-content/70">{{ $t('sidecar.summary', { count: files.length }) }}</p>

      <div class="space-y-2">
        <button
          type="button"
          class="w-full p-3 text-left rounded-box border transition-colors"
          :class="direction === 'export'
            ? 'border-primary bg-primary/10'
            : 'border-neutral-content/30 hover:bg-base-content/5'"
          @click="direction = 'export'"
        >
          <div class="text-sm font-medium">{{ $t('sidecar.export') }}</div>
          <div class="mt-1 text-xs text-base-content/50 leading-relaxed">{{ $t('sidecar.export_hint') }}</div>
        </button>

        <button
          type="button"
          class="w-full p-3 text-left rounded-box border transition-colors"
          :class="direction === 'import'
            ? 'border-primary bg-primary/10'
            : 'border-neutral-content/30 hover:bg-base-content/5'"
          @click="direction = 'import'"
        >
          <div class="text-sm font-medium">{{ $t('sidecar.import') }}</div>
          <div class="mt-1 text-xs text-base-content/50 leading-relaxed">{{ $t('sidecar.import_hint') }}</div>
        </button>
      </div>

      <p class="text-xs text-base-content/50 leading-relaxed">{{ $t('sidecar.naming') }}</p>
    </section>

    <!-- progress -->
    <section v-else class="space-y-3">
      <p class="text-sm">{{ completed ? completionText : $t('sidecar.working') }}</p>
      <progress
        class="progress progress-primary w-full"
        :value="progress.processed"
        :max="Math.max(progress.total, 1)"
      ></progress>
      <p class="text-xs tabular-nums text-base-content/50">
        {{ $t('sidecar.counts', {
          processed: progress.processed,
          total: progress.total,
          changed: progress.changed,
          skipped: progress.skipped,
          failed: progress.failed,
        }) }}
      </p>
      <ul v-if="errors.length" class="max-h-24 overflow-y-auto text-xs text-error space-y-1">
        <li v-for="(message, index) in errors" :key="index" class="truncate" :title="message">{{ message }}</li>
      </ul>
    </section>

    <div class="mt-5 flex justify-end gap-3">
      <button v-if="!completed" class="t-button-default" :disabled="cancelling" @click="close">
        {{ running ? $t('sidecar.stop') : $t('msgbox.cancel') }}
      </button>
      <button v-if="completed" class="t-button-primary" @click="$emit('cancel')">{{ $t('msgbox.ok') }}</button>
      <button v-else class="t-button-primary" :disabled="running || files.length === 0" @click="start">
        {{ $t('sidecar.start') }}
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
  cancelMetadataSidecars,
  listenSidecarFinished,
  listenSidecarProgress,
  transferMetadataSidecars,
} from '@/common/api';

const props = defineProps<{ files: any[] }>();
const emit = defineEmits(['cancel', 'complete']);

const { t } = useI18n();
const toast = useToast();
const uiStore = useUIStore();

const direction = ref<'export' | 'import'>('export');
const running = ref(false);
const cancelling = ref(false);
const completed = ref(false);
const errors = ref<string[]>([]);
const progress = ref({
  processed: 0,
  total: props.files.length,
  changed: 0,
  skipped: 0,
  failed: 0,
  cancelled: false,
});

let unlistenProgress: (() => void) | undefined;
let unlistenFinished: (() => void) | undefined;

const completionText = computed(() => {
  if (progress.value.cancelled) return t('sidecar.done_cancelled', { processed: progress.value.processed });
  return direction.value === 'export'
    ? t('sidecar.done_export', { changed: progress.value.changed, skipped: progress.value.skipped })
    : t('sidecar.done_import', { changed: progress.value.changed, skipped: progress.value.skipped });
});

async function start() {
  running.value = true;
  completed.value = false;
  errors.value = [];
  progress.value = {
    processed: 0,
    total: props.files.length,
    changed: 0,
    skipped: 0,
    failed: 0,
    cancelled: false,
  };

  unlistenProgress = await listenSidecarProgress((event: any) => {
    progress.value = { ...progress.value, ...(event.payload || {}) };
  });

  try {
    let resolveFinished: (value: any) => void = () => {};
    let rejectFinished: (reason?: any) => void = () => {};
    const finished = new Promise<any>((resolve, reject) => {
      resolveFinished = resolve;
      rejectFinished = reject;
    });
    unlistenFinished = await listenSidecarFinished((event: any) => {
      const payload = event.payload || {};
      if (payload.error) rejectFinished(new Error(payload.error));
      else resolveFinished(payload.result);
    });

    await transferMetadataSidecars(
      props.files.map((file) => Number(file.id)).filter(Boolean),
      direction.value,
    );

    const result = await finished;
    progress.value = { ...progress.value, ...result };
    errors.value = Array.isArray(result?.errors) ? result.errors : [];
    completed.value = true;
    emit('complete', { ...result, direction: direction.value });
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
      await cancelMetadataSidecars();
    } finally {
      cancelling.value = false;
    }
    return;
  }
  emit('cancel');
}

onMounted(() => uiStore.pushInputHandler('SidecarDialog'));

onBeforeUnmount(() => {
  uiStore.removeInputHandler('SidecarDialog');
  unlistenProgress?.();
  unlistenFinished?.();
});
</script>
