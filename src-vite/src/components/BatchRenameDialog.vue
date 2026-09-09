<template>
  <ModalDialog :title="$t('rename.title')" :width="600" @cancel="close">
    <!-- template and preview -->
    <section v-if="!running && !completed" class="space-y-4">
      <p class="text-sm text-base-content/70">{{ $t('rename.summary', { count: files.length }) }}</p>

      <div class="grid grid-cols-[1fr_auto] gap-3">
        <div class="space-y-1">
          <label class="text-[10px] uppercase tracking-widest font-bold text-base-content/30 select-none">
            {{ $t('rename.template') }}
          </label>
          <input
            ref="templateInput"
            v-model="namePattern"
            :placeholder="EXAMPLE_PATTERN"
            class="w-full h-8 px-2 text-sm rounded-box bg-base-100 border border-neutral-content/30 focus:border-primary focus:outline-none"
          />
        </div>
        <div class="space-y-1 w-28">
          <label class="text-[10px] uppercase tracking-widest font-bold text-base-content/30 select-none">
            {{ $t('rename.start_index') }}
          </label>
          <input
            v-model.number="startIndex"
            type="number"
            min="0"
            max="99999"
            class="w-full h-8 px-2 text-sm rounded-box bg-base-100 border border-neutral-content/30 focus:border-primary focus:outline-none"
          />
        </div>
      </div>

      <!-- tokens -->
      <div class="space-y-1">
        <div class="text-[10px] uppercase tracking-widest font-bold text-base-content/30 select-none">
          {{ $t('rename.tokens') }}
        </div>
        <div class="flex flex-wrap gap-1">
          <button
            v-for="item in TOKENS"
            :key="item.token"
            type="button"
            class="px-2 py-0.5 text-xs rounded-box border border-neutral-content/30 hover:border-primary hover:text-primary transition-colors"
            :title="$t('rename.token_' + item.key)"
            @click="appendToken(item.token)"
          >
            {{ item.token }}
          </button>
        </div>
      </div>

      <!-- preview -->
      <div class="space-y-1">
        <div class="text-[10px] uppercase tracking-widest font-bold text-base-content/30 select-none">
          {{ $t('rename.preview') }}
        </div>
        <div class="rounded-box border border-base-content/5 bg-base-100/30 overflow-hidden">
          <div class="max-h-48 overflow-y-auto">
            <table class="w-full text-xs">
              <tbody>
                <tr v-for="row in preview" :key="row.fileId" class="border-b border-base-content/5 last:border-0">
                  <td class="px-2 py-1 text-base-content/50 truncate max-w-[240px]" :title="row.oldName">
                    {{ row.oldName }}
                  </td>
                  <td class="px-1 py-1 text-base-content/30 w-4">→</td>
                  <td
                    class="px-2 py-1 truncate max-w-[240px]"
                    :class="row.error ? 'text-error' : 'text-base-content'"
                    :title="row.error || row.newName"
                  >
                    {{ row.error ? row.error : row.newName }}
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
        <p v-if="files.length > preview.length" class="text-xs text-base-content/50">
          {{ $t('rename.preview_truncated', { shown: preview.length, total: files.length }) }}
        </p>
      </div>

      <p class="text-xs text-base-content/50 leading-relaxed">{{ $t('rename.hint') }}</p>
    </section>

    <!-- progress -->
    <section v-else class="space-y-3">
      <p class="text-sm">{{ completed ? completionText : $t('rename.working') }}</p>
      <progress
        class="progress progress-primary w-full"
        :value="progress.processed"
        :max="Math.max(progress.total, 1)"
      ></progress>
      <p class="text-xs tabular-nums text-base-content/50">
        {{ $t('rename.counts', {
          processed: progress.processed,
          total: progress.total,
          renamed: progress.renamed,
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
        {{ running ? $t('rename.stop') : $t('msgbox.cancel') }}
      </button>
      <button v-if="completed" class="t-button-primary" @click="$emit('cancel')">{{ $t('msgbox.ok') }}</button>
      <button v-else class="t-button-primary" :disabled="!canStart" @click="start">
        {{ $t('rename.start') }}
      </button>
    </div>
  </ModalDialog>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import ModalDialog from '@/components/ModalDialog.vue';
import { useToast } from '@/common/toast';
import { useUIStore } from '@/stores/uiStore';
import {
  batchRename,
  cancelBatchRename,
  listenRenameFinished,
  listenRenameProgress,
  previewBatchRename,
} from '@/common/api';

const props = defineProps<{ files: any[] }>();
const emit = defineEmits(['cancel', 'complete']);

const { t } = useI18n();
const toast = useToast();
const uiStore = useUIStore();

/// Clicking one appends it to the pattern. `key` names its help text.
const TOKENS = [
  { token: '{name}', key: 'name' },
  { token: '{n:3}', key: 'n' },
  { token: '{date}', key: 'date' },
  { token: '{year}', key: 'year' },
  { token: '{month}', key: 'month' },
  { token: '{day}', key: 'day' },
  { token: '{time}', key: 'time' },
  { token: '{camera}', key: 'camera' },
  { token: '{lens}', key: 'lens' },
];

/// Shown when the field is empty. Kept out of the translations because
/// vue-i18n would read its braces as interpolation placeholders.
const EXAMPLE_PATTERN = '{date}_{n:3}';

/// The preview asks the backend, so it can only cover so many rows cheaply.
const PREVIEW_LIMIT = 60;

const namePattern = ref(EXAMPLE_PATTERN);
const startIndex = ref(1);
const preview = ref<any[]>([]);
const running = ref(false);
const cancelling = ref(false);
const completed = ref(false);
const errors = ref<string[]>([]);
const templateInput = ref<HTMLInputElement | null>(null);
const progress = ref({
  processed: 0,
  total: props.files.length,
  renamed: 0,
  skipped: 0,
  failed: 0,
  cancelled: false,
});

let unlistenProgress: (() => void) | undefined;
let unlistenFinished: (() => void) | undefined;
let previewTimer: ReturnType<typeof setTimeout> | undefined;

const canStart = computed(
  () => !running.value && namePattern.value.trim().length > 0 && preview.value.some((row) => !row.error),
);

const completionText = computed(() => {
  if (progress.value.cancelled) return t('rename.done_cancelled', { renamed: progress.value.renamed });
  if (progress.value.failed > 0) {
    return t('rename.done_with_failures', {
      renamed: progress.value.renamed,
      failed: progress.value.failed,
    });
  }
  return t('rename.done', { renamed: progress.value.renamed, skipped: progress.value.skipped });
});

function appendToken(token: string) {
  namePattern.value += token;
}

async function refreshPreview() {
  const ids = props.files.slice(0, PREVIEW_LIMIT).map((file) => Number(file.id)).filter(Boolean);
  if (ids.length === 0 || !namePattern.value.trim()) {
    preview.value = [];
    return;
  }
  const rows = await previewBatchRename(ids, namePattern.value, startIndex.value);
  preview.value = Array.isArray(rows) ? rows : [];
}

// Typing should not fire a query per keystroke.
watch([namePattern, startIndex], () => {
  clearTimeout(previewTimer);
  previewTimer = setTimeout(() => void refreshPreview(), 200);
});

async function start() {
  if (!canStart.value) return;
  running.value = true;
  completed.value = false;
  errors.value = [];
  progress.value = {
    processed: 0,
    total: props.files.length,
    renamed: 0,
    skipped: 0,
    failed: 0,
    cancelled: false,
  };

  unlistenProgress = await listenRenameProgress((event: any) => {
    progress.value = { ...progress.value, ...(event.payload || {}) };
  });

  try {
    let resolveFinished: (value: any) => void = () => {};
    let rejectFinished: (reason?: any) => void = () => {};
    const finished = new Promise<any>((resolve, reject) => {
      resolveFinished = resolve;
      rejectFinished = reject;
    });
    unlistenFinished = await listenRenameFinished((event: any) => {
      const payload = event.payload || {};
      if (payload.error) rejectFinished(new Error(payload.error));
      else resolveFinished(payload.result);
    });

    await batchRename(
      props.files.map((file) => Number(file.id)).filter(Boolean),
      namePattern.value,
      startIndex.value,
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
      await cancelBatchRename();
    } finally {
      cancelling.value = false;
    }
    return;
  }
  emit('cancel');
}

onMounted(async () => {
  uiStore.pushInputHandler('BatchRenameDialog');
  await refreshPreview();
  await nextTick();
  templateInput.value?.focus();
});

onBeforeUnmount(() => {
  uiStore.removeInputHandler('BatchRenameDialog');
  clearTimeout(previewTimer);
  unlistenProgress?.();
  unlistenFinished?.();
});
</script>
