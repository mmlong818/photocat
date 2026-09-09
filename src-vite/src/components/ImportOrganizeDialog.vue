<template>
  <ModalDialog :title="$t('import_organize.import')" :width="520" @cancel="close">
    <div class="space-y-4 text-sm">
      <section v-if="!running && !completed" class="space-y-2">
        <div class="font-medium">{{ $t('import_organize.source') }}</div>
        <div class="flex gap-2">
          <div class="min-w-0 flex-1 h-9 px-3 flex items-center rounded-box bg-base-100/60 border border-base-content/10 truncate text-base-content/60">
            {{ sourcePath || $t('import_organize.source_placeholder') }}
          </div>
          <button class="t-button-default shrink-0" @click="chooseSource">{{ $t('import_organize.choose_folder') }}</button>
        </div>
      </section>

      <section v-if="!running && !completed" class="space-y-2">
        <div class="font-medium">{{ $t('import_organize.destination') }}</div>
        <div class="max-h-44 overflow-y-auto rounded-box bg-base-100/40 border border-base-content/10 p-1">
          <AlbumFolder
            :children="destinationTree ? [destinationTree] : []"
            :albumId="Number(album.id)"
            :rootPath="album.path"
            :allowContextMenu="false"
          />
        </div>
      </section>

      <section v-if="!running && !completed" class="space-y-2">
        <label class="font-medium" for="import-layout">{{ $t('import_organize.folder_layout') }}</label>
        <select id="import-layout" v-model="layout" class="select select-bordered w-full h-9 min-h-0">
          <option value="day">{{ $t('import_organize.layout_day') }}</option>
          <option value="month">{{ $t('import_organize.layout_month') }}</option>
          <option value="year">{{ $t('import_organize.layout_year') }}</option>
          <option value="none">{{ $t('import_organize.layout_none') }}</option>
        </select>
      </section>

      <section v-if="running || completed" class="space-y-3">
        <div class="flex items-center justify-between">
          <span class="font-medium">{{ completed ? $t(cancelled ? 'import_organize.cancelled' : 'import_organize.complete') : progress.phase === 'preparing' ? $t('import_organize.preparing') : $t('import_organize.importing') }}</span>
          <span>{{ progress.processed.toLocaleString() }} / {{ progress.total.toLocaleString() }}</span>
        </div>
        <progress class="progress progress-primary w-full" :value="progress.processed" :max="Math.max(progress.total, 1)"></progress>
        <div v-if="progress.currentPath" class="rounded-box bg-base-100/40 border border-base-content/10 px-3 py-2 text-xs truncate" :title="progress.currentPath">
          {{ progress.currentPath }}
        </div>
        <div v-if="progress.failed" class="text-base-content/60">
          {{ $t('import_organize.failed', { count: progress.failed.toLocaleString() }) }}
        </div>
      </section>
    </div>
    <div class="mt-5 flex justify-end gap-3">
      <button v-if="!running" class="t-button-default" @click="close">{{ completed ? $t('msgbox.close') : $t('msgbox.cancel') }}</button>
      <button v-if="!running && !completed" class="t-button-primary" :disabled="!sourcePath" @click="startImport">{{ $t('import_organize.import') }}</button>
      <button v-if="running" class="t-button-default" :disabled="cancelling" @click="cancel">{{ $t('msgbox.cancel') }}</button>
    </div>
  </ModalDialog>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { useToast } from '@/common/toast';
import { useUIStore } from '@/stores/uiStore';
import { openFolderDialog } from '@/common/utils';
import { cancelImportAndOrganize, fetchFolder, importAndOrganize, listenImportOrganizeFinished, listenImportOrganizeProgress } from '@/common/api';
import { config, libConfig } from '@/common/config';
import { useAlbumSelectionProvider } from '@/composables/useAlbumSelection';
import { listen, type Event } from '@tauri-apps/api/event';
import AlbumFolder from '@/components/AlbumFolder.vue';
import ModalDialog from '@/components/ModalDialog.vue';

const props = defineProps<{ album: any }>();
const emit = defineEmits(['complete', 'cancel']);
const toast = useToast();
const uiStore = useUIStore();
const sourcePath = ref('');
const destinationTree = ref<any>(null);
const layout = ref('day');
const running = ref(false);
const completed = ref(false);
const cancelled = ref(false);
const cancelling = ref(false);
const progress = ref({ phase: 'preparing', currentPath: '', processed: 0, total: 0, imported: 0, failed: 0 });
let unlistenProgress: (() => void) | undefined;
let unlistenFinished: (() => void) | undefined;
let unlistenKeydown: (() => void) | undefined;

useAlbumSelectionProvider('destFolder');
libConfig.destFolder.albumId = Number(props.album.id);
libConfig.destFolder.folderId = null;
libConfig.destFolder.folderPath = props.album.path;
libConfig.destFolder.selected = false;
const destinationPath = computed(() => String(libConfig.destFolder.folderPath || props.album.path));

async function chooseSource() {
  const path = await openFolderDialog();
  if (path) sourcePath.value = path;
}

async function startImport() {
  if (!sourcePath.value || running.value) return;
  running.value = true;
  unlistenProgress = await listenImportOrganizeProgress((event: any) => {
    progress.value = event.payload || progress.value;
  });
  try {
    let resolveFinished: (result: any) => void;
    let rejectFinished: (error: Error) => void;
    const finished = new Promise<any>((resolve, reject) => {
      resolveFinished = resolve;
      rejectFinished = reject;
    });
    unlistenFinished = await listenImportOrganizeFinished((event: any) => {
        const payload = event.payload || {};
        payload.error ? rejectFinished(new Error(payload.error)) : resolveFinished(payload.result);
    });
    await importAndOrganize(props.album.id, sourcePath.value, destinationPath.value, layout.value);
    const result = await finished;
    progress.value = { ...progress.value, ...result };
    cancelled.value = Boolean(result.cancelled);
    completed.value = true;
    emit('complete', result);
  } catch (error) {
    toast.error(String(error));
  } finally {
    running.value = false;
    unlistenProgress?.();
    unlistenProgress = undefined;
    unlistenFinished?.();
    unlistenFinished = undefined;
  }
}

async function cancel() {
  if (cancelling.value) return;
  cancelling.value = true;
  try { await cancelImportAndOrganize(); } finally { cancelling.value = false; }
}

function close() {
  if (running.value) return;
  emit('cancel');
}

onBeforeUnmount(() => {
  unlistenProgress?.();
  unlistenFinished?.();
  unlistenKeydown?.();
  uiStore.removeInputHandler('ImportOrganizeDialog');
  libConfig.destFolder.albumId = null;
  libConfig.destFolder.folderId = null;
  libConfig.destFolder.folderPath = null;
  libConfig.destFolder.selected = false;
});

onMounted(async () => {
  uiStore.pushInputHandler('ImportOrganizeDialog');
  unlistenKeydown = await listen<{ key: string }>('global-keydown', (event: Event<{ key: string }>) => {
    if (!uiStore.isInputActive('ImportOrganizeDialog') || event.payload.key !== 'Escape') return;
    if (running.value) {
      void cancel();
    } else {
      close();
    }
  });
  destinationTree.value = await fetchFolder(props.album.path, false, config.settings.folderSort);
});
</script>
