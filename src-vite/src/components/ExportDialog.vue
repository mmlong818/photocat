<template>
  <ModalDialog :title="$t('export.title')" :width="560" @cancel="close">
    <!-- settings -->
    <section v-if="!running && !completed" class="space-y-4">
      <p class="text-sm text-base-content/70">
        {{ $t('export.summary', { count: files.length }) }}
      </p>

      <!-- preset -->
      <div class="space-y-1">
        <label class="text-[10px] uppercase tracking-widest font-bold text-base-content/30 select-none">
          {{ $t('export.preset') }}
        </label>
        <div class="flex items-center gap-2">
          <select
            v-model="presetId"
            class="grow h-8 px-2 text-sm rounded-box bg-base-100 border border-neutral-content/30 focus:border-primary focus:outline-none"
            @change="applyPreset"
          >
            <option v-for="option in presetOptions" :key="option.id" :value="option.id">
              {{ option.name }}
            </option>
          </select>
          <TButton :icon="IconAdd" :tooltip="$t('export.save_preset')" @click="startSavePreset" />
          <TButton
            :icon="IconTrash"
            :tooltip="$t('export.delete_preset')"
            :disabled="!isUserPreset"
            @click="deletePreset"
          />
        </div>
      </div>

      <!-- naming a new preset -->
      <div v-if="savingPreset" class="flex items-center gap-2">
        <input
          ref="presetNameInput"
          v-model="presetName"
          :maxlength="PRESET_NAME_MAX"
          :placeholder="$t('export.preset_name_placeholder')"
          class="grow h-8 px-2 text-sm rounded-box bg-base-100 border border-neutral-content/30 focus:border-primary focus:outline-none"
          @keydown.enter.prevent="commitSavePreset"
          @keydown.esc.prevent="savingPreset = false"
        />
        <button class="t-button-primary" :disabled="!presetName.trim()" @click="commitSavePreset">
          {{ $t('msgbox.ok') }}
        </button>
      </div>

      <!-- format and quality -->
      <div class="grid grid-cols-2 gap-3">
        <div class="space-y-1">
          <label class="text-[10px] uppercase tracking-widest font-bold text-base-content/30 select-none">
            {{ $t('export.format') }}
          </label>
          <select
            v-model="format"
            class="w-full h-8 px-2 text-sm rounded-box bg-base-100 border border-neutral-content/30 focus:border-primary focus:outline-none"
          >
            <option value="jpeg">JPEG</option>
            <option value="png">PNG</option>
            <option value="webp">WebP</option>
          </select>
        </div>
        <div class="space-y-1">
          <label class="text-[10px] uppercase tracking-widest font-bold text-base-content/30 select-none">
            {{ $t('export.quality') }}
          </label>
          <select
            v-model.number="quality"
            :disabled="format !== 'jpeg'"
            class="w-full h-8 px-2 text-sm rounded-box bg-base-100 border border-neutral-content/30 focus:border-primary focus:outline-none disabled:opacity-40"
          >
            <option v-for="option in EXPORT_QUALITY_CHOICES" :key="option.value" :value="option.value">
              {{ $t('export.quality_' + option.key) }}
            </option>
          </select>
        </div>
      </div>
      <p v-if="format !== 'jpeg'" class="-mt-2 text-xs text-base-content/50">
        {{ $t('export.quality_lossless') }}
      </p>

      <!-- size -->
      <div class="space-y-1">
        <label class="text-[10px] uppercase tracking-widest font-bold text-base-content/30 select-none">
          {{ $t('export.size') }}
        </label>
        <select
          v-model.number="maxEdge"
          class="w-full h-8 px-2 text-sm rounded-box bg-base-100 border border-neutral-content/30 focus:border-primary focus:outline-none"
        >
          <option v-for="edge in EXPORT_EDGE_CHOICES" :key="edge" :value="edge">
            {{ edge === 0 ? $t('export.size_original') : $t('export.size_long_edge', { px: edge }) }}
          </option>
        </select>
        <p class="text-xs text-base-content/50">{{ $t('export.size_hint') }}</p>
      </div>

      <!-- metadata -->
      <div class="flex items-center justify-between p-1 rounded-box hover:bg-base-100/10">
        <div class="flex flex-col gap-0.5 text-sm leading-5">
          <div>{{ $t('export.keep_metadata') }}</div>
          <div class="text-xs text-base-content/50">{{ $t('export.keep_metadata_hint') }}</div>
        </div>
        <input
          v-model="keepMetadata"
          type="checkbox"
          class="toggle toggle-primary toggle-sm shrink-0"
          :disabled="format === 'png'"
        />
      </div>

      <!-- destination -->
      <div class="space-y-1">
        <label class="text-[10px] uppercase tracking-widest font-bold text-base-content/30 select-none">
          {{ $t('export.destination') }}
        </label>
        <div class="flex items-center gap-2">
          <div
            class="grow h-8 px-2 flex items-center text-sm rounded-box bg-base-100 border border-neutral-content/30 overflow-hidden"
            :class="destination ? '' : 'text-base-content/30'"
          >
            <span class="truncate" :title="destination">
              {{ destination || $t('export.destination_placeholder') }}
            </span>
          </div>
          <button class="t-button-default shrink-0" @click="pickDestination">
            {{ $t('export.browse') }}
          </button>
        </div>
      </div>

      <!-- conflicts -->
      <div class="space-y-1">
        <label class="text-[10px] uppercase tracking-widest font-bold text-base-content/30 select-none">
          {{ $t('export.conflict') }}
        </label>
        <div class="flex gap-2">
          <button
            v-for="option in conflictOptions"
            :key="option.value"
            type="button"
            class="flex-1 h-8 text-sm rounded-box border transition-colors"
            :class="conflict === option.value
              ? 'border-primary text-primary bg-primary/10'
              : 'border-neutral-content/30 text-base-content/70 hover:bg-base-content/5'"
            @click="conflict = option.value"
          >
            {{ option.label }}
          </button>
        </div>
      </div>
    </section>

    <!-- progress -->
    <section v-else class="space-y-3">
      <p class="text-sm">
        {{ completed ? completionText : $t('export.exporting', { name: progress.currentName || '' }) }}
      </p>
      <progress
        class="progress progress-primary w-full"
        :value="progress.processed"
        :max="Math.max(progress.total, 1)"
      ></progress>
      <p class="text-xs tabular-nums text-base-content/50">
        {{ $t('export.counts', {
          processed: progress.processed,
          total: progress.total,
          exported: progress.exported,
          skipped: progress.skipped,
          failed: progress.failed,
        }) }}
      </p>
      <ul v-if="errors.length" class="max-h-24 overflow-y-auto text-xs text-error space-y-1">
        <li v-for="(message, index) in errors" :key="index" class="truncate" :title="message">{{ message }}</li>
      </ul>
    </section>

    <div class="mt-5 flex justify-end gap-3">
      <button v-if="completed" class="t-button-default" @click="revealDestination">
        {{ $t('export.open_folder') }}
      </button>
      <button v-if="!completed" class="t-button-default" :disabled="cancelling" @click="close">
        {{ running ? $t('export.stop') : $t('msgbox.cancel') }}
      </button>
      <button v-if="completed" class="t-button-primary" @click="$emit('cancel')">
        {{ $t('msgbox.ok') }}
      </button>
      <button v-else class="t-button-primary" :disabled="!canStart" @click="start">
        {{ $t('export.start') }}
      </button>
    </div>
  </ModalDialog>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import ModalDialog from '@/components/ModalDialog.vue';
import TButton from '@/components/TButton.vue';
import { IconAdd, IconTrash } from '@/common/icons';
import { useToast } from '@/common/toast';
import { useUIStore } from '@/stores/uiStore';
import { cancelExport, exportFiles, listenExportFinished, listenExportProgress, revealPath } from '@/common/api';
import { openFolderDialog } from '@/common/utils';
import {
  BUILT_IN_PRESETS,
  EXPORT_EDGE_CHOICES,
  EXPORT_QUALITY_CHOICES,
  PRESET_NAME_MAX,
  findPreset,
  getConflictPolicy,
  getLastDestination,
  getLastPresetId,
  listPresets,
  removePreset,
  savePreset,
  setConflictPolicy,
  setLastDestination,
  setLastPresetId,
  type ConflictPolicy,
  type ExportFormat,
} from '@/common/exportPresets';

const props = defineProps<{ files: any[] }>();
const emit = defineEmits(['cancel', 'complete']);

const { t } = useI18n();
const toast = useToast();
const uiStore = useUIStore();

const presetId = ref(getLastPresetId());
const format = ref<ExportFormat>('jpeg');
const quality = ref(80);
const maxEdge = ref(2048);
const keepMetadata = ref(false);
const destination = ref(getLastDestination());
const conflict = ref<ConflictPolicy>(getConflictPolicy());

const savingPreset = ref(false);
const presetName = ref('');
const presetNameInput = ref<HTMLInputElement | null>(null);

const running = ref(false);
const cancelling = ref(false);
const completed = ref(false);
const errors = ref<string[]>([]);
const progress = ref({
  currentName: '',
  processed: 0,
  total: props.files.length,
  exported: 0,
  skipped: 0,
  failed: 0,
});

let unlistenProgress: (() => void) | undefined;
let unlistenFinished: (() => void) | undefined;

const presetOptions = computed(() => [
  ...BUILT_IN_PRESETS.map((preset) => ({
    id: preset.id,
    name: t('export.preset_' + preset.nameKey),
  })),
  ...listPresets().map((preset) => ({ id: preset.id, name: preset.name })),
]);

const isUserPreset = computed(() => presetId.value.startsWith('user:'));

const conflictOptions = computed<{ value: ConflictPolicy; label: string }[]>(() => [
  { value: 'keep_both', label: t('export.conflict_keep_both') },
  { value: 'replace', label: t('export.conflict_replace') },
  { value: 'skip', label: t('export.conflict_skip') },
]);

const canStart = computed(() => props.files.length > 0 && !!destination.value && !running.value);

const completionText = computed(() => {
  if (progress.value.cancelled) return t('export.done_cancelled', { exported: progress.value.exported });
  if (progress.value.failed > 0) {
    return t('export.done_with_failures', {
      exported: progress.value.exported,
      failed: progress.value.failed,
    });
  }
  return t('export.done', { exported: progress.value.exported });
});

function applyPreset() {
  const builtIn = BUILT_IN_PRESETS.find((preset) => preset.id === presetId.value);
  const source = builtIn ?? findPreset(presetId.value);
  if (!source) return;
  format.value = source.format;
  quality.value = source.quality;
  maxEdge.value = source.maxEdge;
  keepMetadata.value = source.keepMetadata;
  setLastPresetId(presetId.value);
}

async function startSavePreset() {
  savingPreset.value = true;
  presetName.value = '';
  await nextTick();
  presetNameInput.value?.focus();
}

function commitSavePreset() {
  const saved = savePreset({
    name: presetName.value,
    format: format.value,
    quality: quality.value,
    maxEdge: maxEdge.value,
    keepMetadata: keepMetadata.value,
  });
  if (!saved) {
    toast.error(t('export.preset_save_failed'));
    return;
  }
  presetId.value = saved.id;
  setLastPresetId(saved.id);
  savingPreset.value = false;
}

function deletePreset() {
  if (!isUserPreset.value) return;
  removePreset(presetId.value);
  presetId.value = getLastPresetId();
  applyPreset();
}

async function pickDestination() {
  const picked = await openFolderDialog(t('export.destination'));
  if (!picked) return;
  destination.value = String(picked);
  setLastDestination(destination.value);
}

async function revealDestination() {
  if (destination.value) await revealPath(destination.value);
}

async function start() {
  if (!canStart.value) return;
  running.value = true;
  completed.value = false;
  errors.value = [];
  setConflictPolicy(conflict.value);
  setLastDestination(destination.value);
  progress.value = {
    currentName: '',
    processed: 0,
    total: props.files.length,
    exported: 0,
    skipped: 0,
    failed: 0,
  };

  unlistenProgress = await listenExportProgress((event: any) => {
    progress.value = { ...progress.value, ...(event.payload || {}) };
  });

  try {
    let resolveFinished: (value: any) => void = () => {};
    let rejectFinished: (reason?: any) => void = () => {};
    const finished = new Promise<any>((resolve, reject) => {
      resolveFinished = resolve;
      rejectFinished = reject;
    });
    unlistenFinished = await listenExportFinished((event: any) => {
      const payload = event.payload || {};
      if (payload.error) rejectFinished(new Error(payload.error));
      else resolveFinished(payload.result);
    });

    await exportFiles({
      files: props.files.map((file) => String(file.file_path)).filter(Boolean),
      destination: destination.value,
      preset: {
        format: format.value,
        quality: quality.value,
        maxEdge: maxEdge.value,
        keepMetadata: keepMetadata.value,
      },
      conflict: conflict.value,
    });

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
    // Stop after the file in flight rather than abandoning a half-written one.
    if (cancelling.value) return;
    cancelling.value = true;
    try {
      await cancelExport();
    } finally {
      cancelling.value = false;
    }
    return;
  }
  emit('cancel');
}

onMounted(() => {
  uiStore.pushInputHandler('ExportDialog');
  applyPreset();
});

onBeforeUnmount(() => {
  uiStore.removeInputHandler('ExportDialog');
  unlistenProgress?.();
  unlistenFinished?.();
});
</script>
