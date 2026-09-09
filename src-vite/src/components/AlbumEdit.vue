<template>
  <ModalDialog :title="isNewAlbum ? $t('album.edit.title_add') : $t('album.edit.title')" @cancel="clickCancel">
    <section class="space-y-2">

      <!-- General Information -->
      <div class="rounded-box p-2 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
        <div class="flex items-center gap-2 text-base-content/30">
          <span class="font-bold uppercase text-[10px] tracking-widest">{{ $t('album.edit.section_general') }}</span>
        </div>
        <div class="w-full grid grid-cols-[84px_1fr] gap-x-4 gap-y-1.5 items-center px-1 text-xs select-none">
          <!-- Folder -->
          <div class="h-6 flex items-center text-[11px] text-base-content/45">{{ $t('album.edit.folder') }}</div>
          <div class="min-h-6 flex flex-col justify-center gap-0.5">
            <div class="flex items-center justify-between gap-x-2">
            <input v-if="selectedFolder !== ''"
              type="text"
              readonly
              :value="selectedFolder"
              class="w-full bg-transparent border-none p-0 text-[12px] text-base-content/75 focus:border-none focus:ring-0 focus:outline-none"
            />
            <button v-if="selectedFolder === ''"
              class="btn btn-primary btn-sm rounded-box"
              @click="clickSelectFolder"
            >
              <IconNewFolder class="w-4 h-4" />
              {{ $t('album.edit.select_folder') }}
            </button>
            <TButton v-if="isNewAlbum && selectedFolder !== ''"
              :icon="IconNewFolder"
              :selected="true"
              @click="clickSelectFolder"
            />
            </div>
            <div
              v-if="!isNewAlbum && album?.is_accessible === false"
              class="text-[11px] leading-4 text-warning"
            >
              {{ $t('album.folder_unavailable.title') }}
            </div>
          </div>

          <!-- Name -->
          <div class="h-6 flex items-center text-[11px] text-base-content/45">{{ $t('album.edit.name') }}</div>
          <div class="flex min-h-6 items-center">
            <input
              ref="inputNameRef"
              v-model="inputNameValue"
              type="text"
              maxlength="255"
              :disabled="selectedFolder === ''"
              class="w-full input input-xs h-6 px-1.5 text-[12px] font-medium"
            />
          </div>

          <!-- Description -->
          <div class="h-6 flex items-start pt-1 text-[11px] text-base-content/45">{{ $t('album.edit.description') }}</div>
          <div>
            <textarea
              v-if="showDescription"
              ref="descriptionRef"
              v-model="inputDescriptionValue"
              rows="2"
              maxlength="1024"
              :placeholder="$t('album.edit.description_placeholder')"
              :disabled="selectedFolder === ''"
              class="w-full textarea textarea-sm min-h-14 max-h-50 px-1.5 text-[12px] font-medium"
            ></textarea>
            <TButton
              v-else
              :icon="IconEdit"
              :buttonSize="'small'"
              :tooltip="$t('album.edit.description')"
              :disabled="selectedFolder === ''"
              @click="showDescriptionInput"
            />
          </div>

          <template v-if="!isNewAlbum">
            <div class="h-6 flex items-center text-[11px] text-base-content/45">{{ $t('album.edit.created_at') }}</div>
            <div class="h-6 flex items-center text-[12px] text-base-content/75">{{ createdAt }}</div>
            <div class="h-6 flex items-center text-[11px] text-base-content/45">{{ $t('album.edit.modified_at') }}</div>
            <div class="h-6 flex items-center text-[12px] text-base-content/75">{{ modifiedAt }}</div>
          </template>
        </div>
      </div>

      <!-- Scan Status -->
      <div v-if="selectedFolder !== ''" class="rounded-box p-2 space-y-2 bg-base-300/30 border border-base-content/5 shadow-sm">
        <div class="flex items-center gap-2 text-base-content/30">
          <span class="font-bold uppercase text-[10px] tracking-widest">{{ $t('album.edit.section_scan') }}</span>
        </div>
        <div class="w-full grid grid-cols-[84px_1fr] gap-x-4 gap-y-1.5 items-center px-1 text-xs select-none">
          <div class="h-6 flex items-center text-[11px] text-base-content/45">
            {{ isNewAlbum ? $t('album.edit.files_to_scan') : (isScanning ? $t('album.edit.scanning') : $t('album.edit.scanned_files')) }}
          </div>
          <div class="h-6 flex items-center text-[12px] text-base-content/75" :class="{ 'animate-pulse': scanDisplayCount < 0 }">
            <template v-if="isScanning">
              {{ $t('album.edit.scanning_files', { current: scanDisplayCount.toLocaleString(), total: scanTotalCount.toLocaleString(), size: formatFileSize(scanTotalSize) }) }}
            </template>
            <template v-else>
              {{ scanDisplayCount >= 0 ? $t('album.edit.files_count', { count: scanDisplayCount.toLocaleString(), size: formatFileSize(scanDisplaySize) }) : $t('album.edit.files_counting') }}
            </template>
          </div>
          <template v-if="!isNewAlbum || isScanning">
          <div v-if="indexedSummaryCount > 0" class="h-6 flex items-center text-[11px] text-base-content/45">{{ $t('album.edit.indexed_files') }}</div>
          <div v-if="indexedSummaryCount > 0" class="h-6 flex items-center text-[12px] text-base-content/75">
            {{ $t('album.edit.files_count', { count: indexedSummaryCount.toLocaleString(), size: formatFileSize(indexedSummarySize) }) }}
          </div>
          <div v-if="displaySkippedCount > 0" class="h-6 flex items-center text-[11px] text-base-content/45">{{ $t('album.edit.skipped_files') }}</div>
          <div v-if="displaySkippedCount > 0" class="h-6 flex items-center text-[12px] text-base-content/75">
            {{ $t('album.edit.files_count', { count: displaySkippedCount.toLocaleString(), size: formatFileSize(displaySkippedSize) }) }}
          </div>
          <div v-if="!isScanning && mergedFileCount > 0" class="h-6 flex items-center text-[11px] text-base-content/45">{{ $t('album.edit.merged_files') }}</div>
          <div v-if="!isScanning && mergedFileCount > 0" class="h-6 flex items-center text-[12px] text-base-content/75">
            {{ formatFileCount(mergedFileCount, mergedFileSize) }}
          </div>
          <div v-if="displayFailedCount > 0" class="h-6 flex items-center text-[11px] text-error/70">{{ $t('album.edit.failed_files') }}</div>
          <div v-if="displayFailedCount > 0" class="h-6 flex items-center text-[12px] text-error/70">
            {{ formatFileCount(displayFailedCount, displayFailedSize) }}
          </div>
          <div v-if="!isScanning" class="h-6 flex items-center text-[11px] text-base-content/45">{{ $t('album.edit.last_scan_time') }}</div>
          <div v-if="!isScanning" class="h-6 flex items-center text-[12px] text-base-content/75">{{ lastScanTime }}</div>
          </template>
        </div>
      </div>
    </section>

    <!-- cancel and OK buttons -->
    <div class="mt-4 flex justify-end space-x-4">
      <button 
        class="t-button-default" 
        @mouseup.left.stop.prevent="clickCancel"
        @click="!$event.detail && clickCancel()"
      >
        {{ $t('msgbox.cancel') }}
      </button>
      <button 
        class="t-button-primary"
        :disabled="inputNameValue.trim().length === 0 || selectedFolder.length === 0"
        @mouseup.left.stop.prevent="clickOk"
        @click="!$event.detail && clickOk()"
      >
        {{ $t('msgbox.ok') }}
      </button>
    </div>
  </ModalDialog>
</template>

<script setup lang="ts">

import { ref, watch, onMounted, onUnmounted, computed, nextTick } from 'vue';
import { useI18n } from 'vue-i18n';
import { countFolder, getAlbum, getAllAlbums, listenIndexProgress, listenIndexFinished } from '@/common/api';
import { useToast } from '@/common/toast';
import { formatFileSize, formatTimestamp, openFolderDialog, getFolderName } from '@/common/utils';
import { useUIStore } from '@/stores/uiStore';
import { useLibraryStore } from '@/stores/libraryStore';
import { getAlbumScanState } from '@/common/scanStatus';

import ModalDialog from '@/components/ModalDialog.vue';
import TButton from '@/components/TButton.vue';
import { IconEdit, IconNewFolder } from '@/common/icons';

const props = defineProps({
  albumId: {
    type: Number,
    required: true
  },
  initialFolderPath: {
    type: String, 
    default: '' 
  },
});

const emit = defineEmits(['ok', 'cancel']);
const uiStore = useUIStore();
const libStore = useLibraryStore();
const { t } = useI18n();
const toast = useToast();
const isNewAlbum = computed(() => props.albumId <= 0);
const album = ref<any>(null);
const createdAt = computed(() => formatTimestamp(Number(album.value?.created_at || 0), t('format.date_time')));
const modifiedAt = computed(() => formatTimestamp(Number(album.value?.modified_at || 0), t('format.date_time')));
const lastScanTime = computed(() => formatTimestamp(Number(album.value?.last_scan_time || 0) / 1000, t('format.date_time')));

// select folder
const selectedFolder = ref('');

// input 
const inputNameRef = ref<HTMLInputElement | null>(null);
const descriptionRef = ref<HTMLTextAreaElement | null>(null);
const inputNameValue = ref('');
const inputDescriptionValue = ref('');
const showDescription = ref(isNewAlbum.value);

// total file count of the album (from disk probe)
const totalImageCount = ref(-1);
const totalImageSize = ref(-1);
const totalVideoCount = ref(0);
const totalVideoSize = ref(0);

// indexing progress
const indexedCount = ref(0);
const totalCount = ref(0);
const discoveredCount = ref(0);
const scannedSize = ref(0);
const skippedCount = ref(0);
const skippedSize = ref(0);
const failedCount = ref(0);
const failedSize = ref(0);
const scanTotalCount = ref(-1);
const scanTotalSize = ref(0);
const skippedFileCount = computed(() => Number(album.value?.skipped_count || 0));
const skippedFileSize = computed(() => Number(album.value?.skipped_size || 0));
const failedFileCount = computed(() => Number(album.value?.failed_count || 0));
const failedFileSize = computed(() => Number(album.value?.failed_size || 0));
const mergedFileCount = computed(() => Number(album.value?.merged_count || 0));
const mergedFileSize = computed(() => Number(album.value?.merged_size || 0));
const isScanning = computed(() => {
  if (isNewAlbum.value) return false;
  return getAlbumScanState({
    albumId: props.albumId,
    albumQueue: libStore.index.albumQueue as any[],
    pausedAlbumIds: libStore.index.pausedAlbumIds as any[],
    status: Number(libStore.index.status || 0),
  }) === 'scanning';
});

const indexedFileCount = computed(() => totalImageCount.value + totalVideoCount.value);
const indexedFileSize = computed(() => totalImageSize.value + totalVideoSize.value);
const scanDisplayCount = computed(() => isNewAlbum.value
  ? scanTotalCount.value
  : isScanning.value
    ? discoveredCount.value + skippedCount.value
    : Number(album.value?.total || 0) + mergedFileCount.value + skippedFileCount.value + failedFileCount.value);
const scanDisplaySize = computed(() => isNewAlbum.value
  ? scanTotalSize.value
  : isScanning.value
    ? scannedSize.value
  : indexedFileSize.value + skippedFileSize.value);
const indexedSummaryCount = computed(() => Math.max(0, isScanning.value
  ? discoveredCount.value - failedCount.value
  : Number(album.value?.total || 0)));
const indexedSummarySize = computed(() => Math.max(0, isScanning.value
  ? scannedSize.value - skippedSize.value - failedSize.value
  : indexedFileSize.value - mergedFileSize.value - failedFileSize.value));
const displaySkippedCount = computed(() => isScanning.value ? skippedCount.value : skippedFileCount.value);
const displaySkippedSize = computed(() => isScanning.value ? skippedSize.value : skippedFileSize.value);
const displayFailedCount = computed(() => isScanning.value ? failedCount.value : failedFileCount.value);
const displayFailedSize = computed(() => isScanning.value ? failedSize.value : failedFileSize.value);

const formatFileCount = (count: number, size: number) => size > 0
  ? t('album.edit.files_count', { count: count.toLocaleString(), size: formatFileSize(size) })
  : t('album.edit.files_count_without_size', { count: count.toLocaleString() });

let unlistenIndexProgress: (() => void) | undefined;
let unlistenIndexFinished: (() => void) | undefined;

watch(() => selectedFolder.value, (newPath) => {
  if(newPath) {
    if (isNewAlbum.value) {
      // get folder name
      inputNameValue.value = getFolderName(newPath);
      inputDescriptionValue.value = '';
      showDescription.value = true;
    }

    countFolder(newPath).then((res) => {
      [, totalImageCount.value, totalImageSize.value, totalVideoCount.value, totalVideoSize.value, scanTotalCount.value, scanTotalSize.value] = res;
      console.log('count folder:', res);
    }).catch((err) => {
      console.error('count folder error:', err);
    });
  }
});

onMounted(async () => {
  window.addEventListener('keydown', handleKeyDown);
  uiStore.pushInputHandler('AlbumEdit');
  
  // listen for index progress
  unlistenIndexProgress = await listenIndexProgress((event: any) => {
    const { album_id, current, discovered, total, current_size, skipped, skipped_size, failed, failed_size, scan_total, scan_total_size } = event.payload;
    if (Number(album_id) === Number(props.albumId)) {
      indexedCount.value = current;
      totalCount.value = total;
      discoveredCount.value = Number(discovered || 0);
      scannedSize.value = Number(current_size || 0);
      skippedCount.value = Number(skipped || 0);
      skippedSize.value = Number(skipped_size || 0);
      failedCount.value = Number(failed || 0);
      failedSize.value = Number(failed_size || 0);
      scanTotalCount.value = Number(scan_total || 0);
      scanTotalSize.value = Number(scan_total_size || 0);
    }
  });

  // listen for index finished
  unlistenIndexFinished = await listenIndexFinished((event: any) => {
    const { album_id } = event.payload;
    if (Number(album_id) === Number(props.albumId)) {
      // Refresh album info if needed? Usually total counts should be updated.
    }
  });

  if (isNewAlbum.value) {
    selectedFolder.value = props.initialFolderPath;
  } else {
    album.value = await getAlbum(props.albumId);
    if (!album.value) return;
    inputNameValue.value = album.value.name || '';
    inputDescriptionValue.value = album.value.description || '';
    showDescription.value = inputDescriptionValue.value.trim().length > 0;
    selectedFolder.value = album.value.path || '';
  }

  if (selectedFolder.value) {
    setTimeout(() => {
      inputNameRef.value?.focus();
    }, 50); // 50ms delay
  }
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeyDown);
  uiStore.removeInputHandler('AlbumEdit');
  
  if (unlistenIndexProgress) unlistenIndexProgress();
  if (unlistenIndexFinished) unlistenIndexFinished();
});

const clickSelectFolder = async () => {
  const folderPath = await openFolderDialog();
  if (folderPath) {
    selectedFolder.value = folderPath;
    // Auto focus name input after folder selected
    setTimeout(() => {
      inputNameRef.value?.focus();
    }, 100);
  }
};

async function showDescriptionInput() {
  showDescription.value = true;
  await nextTick();
  descriptionRef.value?.focus();
}

function handleKeyDown(event: KeyboardEvent) {
  if (!uiStore.isInputActive('AlbumEdit')) return;

  const { key } = event;
  const activeElement = document.activeElement;
  const isInputOrTextarea = activeElement?.tagName === 'INPUT' || activeElement?.tagName === 'TEXTAREA';

  switch (key) {
    case 'Enter':
      // Allow Enter to submit if in a text input (but not a textarea)
      if (activeElement?.tagName === 'INPUT' || !isInputOrTextarea) {
        event.preventDefault();
        clickOk();
      }
      break;
    case 'Escape':
      clickCancel();
      break;
    default:
      break;
  }
}

const clickOk = async () => {
  if (inputNameValue.value.trim().length > 0 && selectedFolder.value.length > 0) {
    // Check if album with this path already exists
    if (isNewAlbum.value) {
      const albums = await getAllAlbums();
      const exists = albums?.some((album: any) => album.path === selectedFolder.value);
      if (exists) {
        toast.warning(t('tooltip.album_exists'));
        return;
      }
    }
    
    emit(
      'ok', 
      selectedFolder.value,
      inputNameValue.value, 
      inputDescriptionValue.value ? inputDescriptionValue.value : '',
      isNewAlbum.value
    );
  }
};

const clickCancel = () => {
  emit('cancel');
};

</script>
