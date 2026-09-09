<template>
  <div class="w-screen h-screen flex flex-col overflow-hidden bg-base-300 text-base-content/70">
    <TitleBar
      v-if="showDesktopTitleBar"
      :titlebar="`${$t('browse.title')} - ${folderName}`"
      viewName="Browse"
      class="shrink-0 z-50"
    />
    <div
      v-else
      class="h-10 shrink-0 flex items-center justify-center px-20 select-none"
      data-tauri-drag-region
    >
      <div class="min-w-0 max-w-full text-center text-sm font-medium truncate" data-tauri-drag-region>
        {{ $t('browse.title') }} - {{ folderName }}
      </div>
    </div>

    <!-- toolbar -->
    <div class="shrink-0 flex items-center gap-2 px-3 py-2 border-b border-base-content/5">
      <button
        class="btn btn-ghost btn-xs"
        :disabled="!listing?.parent"
        :title="$t('browse.up')"
        @click="goUp"
      >
        {{ $t('browse.up') }}
      </button>

      <div class="min-w-0 flex-1 text-xs truncate" :title="currentFolder">{{ currentFolder }}</div>

      <select v-model.number="sort" class="select select-xs w-28" @change="reload">
        <option :value="SORT_NAME">{{ $t('browse.sort_name') }}</option>
        <option :value="SORT_DATE">{{ $t('browse.sort_date') }}</option>
        <option :value="SORT_SIZE">{{ $t('browse.sort_size') }}</option>
        <option :value="SORT_TYPE">{{ $t('browse.sort_type') }}</option>
      </select>
      <button
        class="btn btn-ghost btn-xs w-8"
        :title="descending ? $t('browse.descending') : $t('browse.ascending')"
        @click="toggleOrder"
      >
        {{ descending ? '↓' : '↑' }}
      </button>

      <span class="text-xs opacity-60 whitespace-nowrap">
        {{ $t('browse.file_count', { count: files.length }) }}
      </span>

      <button class="btn btn-primary btn-xs" :disabled="isAdding" @click="addFolderToLibrary">
        <span v-if="isAdding" class="loading loading-spinner loading-xs"></span>
        {{ $t('browse.add_to_library') }}
      </button>
    </div>

    <!-- grid -->
    <div ref="gridRef" class="flex-1 overflow-y-auto p-3" tabindex="0" @keydown="onGridKeydown">
      <div v-if="isLoading" class="h-full flex items-center justify-center">
        <span class="loading loading-spinner loading-lg text-primary"></span>
      </div>

      <div v-else-if="loadError" class="py-16 text-center text-sm">
        <div class="text-error">{{ $t('browse.cannot_open', { folder: currentFolder }) }}</div>
        <div class="mt-2 opacity-50 break-all">{{ loadError }}</div>
      </div>

      <template v-else>
        <div v-if="listing?.folders.length" class="flex flex-wrap gap-2 mb-3">
          <button
            v-for="folder in listing.folders"
            :key="folder.path"
            class="btn btn-ghost btn-sm justify-start max-w-56"
            :title="folder.path"
            @click="goTo(folder.path)"
          >
            <span class="truncate">{{ folder.name }}</span>
          </button>
        </div>

        <div v-if="!files.length" class="py-16 text-center text-sm opacity-50">
          {{ $t('browse.empty') }}
        </div>

        <div v-else class="grid gap-2" :style="gridStyle">
          <div
            v-for="(file, index) in files"
            :key="file.path"
            :ref="(el) => registerCell(el, index)"
            class="cell rounded-box p-1 cursor-pointer border transition-colors"
            :class="index === selected
              ? 'border-primary bg-primary/10'
              : 'border-transparent hover:bg-base-100/40'"
            :title="file.name"
            @click="selected = index"
            @dblclick="openViewer(index)"
          >
            <div class="aspect-square flex items-center justify-center overflow-hidden rounded">
              <img
                :src="browseThumbUrl(file)"
                loading="lazy"
                decoding="async"
                class="max-w-full max-h-full object-contain"
                @error="onThumbError"
              />
            </div>
            <div class="mt-1 text-[11px] leading-tight text-center truncate">{{ file.name }}</div>
          </div>
        </div>
      </template>
    </div>

    <!-- large view -->
    <div
      v-if="viewerIndex >= 0"
      class="fixed inset-0 z-100 bg-base-300 flex flex-col"
      @wheel.prevent="onViewerWheel"
    >
      <div class="shrink-0 flex items-center gap-3 px-3 py-2 text-xs border-b border-base-content/5">
        <button class="btn btn-ghost btn-xs" @click="closeViewer">{{ $t('browse.back') }}</button>
        <div class="min-w-0 flex-1 truncate" :title="current?.path">{{ current?.name }}</div>
        <span class="opacity-60 whitespace-nowrap">{{ viewerIndex + 1 }} / {{ files.length }}</span>
      </div>
      <div class="flex-1 min-h-0 flex items-center justify-center overflow-hidden">
        <img
          v-if="current"
          :key="current.path"
          :src="browseFullUrl(current)"
          class="max-w-full max-h-full object-contain select-none"
          draggable="false"
        />
      </div>
      <div class="shrink-0 flex items-center justify-center gap-2 py-2">
        <button class="btn btn-ghost btn-sm" :disabled="viewerIndex <= 0" @click="step(-1)">←</button>
        <button
          class="btn btn-ghost btn-sm"
          :disabled="viewerIndex >= files.length - 1"
          @click="step(1)"
        >→</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
// Browsing a folder off disk. Nothing here touches the library, which is the
// entire point: opening one photo from Explorer should not turn its folder
// into an album. The one exception is the explicit "add to library" button.
import { ref, computed, onMounted, onUnmounted, nextTick } from 'vue';
import { useI18n } from 'vue-i18n';
import { useRouter } from 'vue-router';
import { emit as tauriEmit, listen } from '@tauri-apps/api/event';
import {
  browseFolder,
  browseThumbUrl,
  browseFullUrl,
  SORT_NAME,
  SORT_DATE,
  SORT_SIZE,
  SORT_TYPE,
  type BrowseFile,
  type BrowseListing,
} from '@/common/browse';
import { addAlbum } from '@/common/api';
import { isWin, isLinux } from '@/common/utils';
import { useToast } from '@/common/toast';
import TitleBar from '@/components/TitleBar.vue';

const { t } = useI18n();
const router = useRouter();
const toast = useToast();
const showDesktopTitleBar = isWin || isLinux;

const listing = ref<BrowseListing | null>(null);
const isLoading = ref(true);
const isAdding = ref(false);
const currentFolder = ref('');
const loadError = ref('');
const sort = ref(SORT_NAME);
const descending = ref(false);
const selected = ref(-1);
const viewerIndex = ref(-1);
const gridRef = ref<HTMLElement | null>(null);
const cells: HTMLElement[] = [];
let unlistenGoto: (() => void) | null = null;

const files = computed<BrowseFile[]>(() => listing.value?.files ?? []);
const current = computed(() => files.value[viewerIndex.value] ?? null);
const folderName = computed(() => {
  const path = currentFolder.value.replace(/[\\/]+$/, '');
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
});
// A fixed cell width keeps the row height predictable, which is what lets the
// browser skip the work for offscreen rows.
const gridStyle = { gridTemplateColumns: 'repeat(auto-fill, minmax(140px, 1fr))' };

function registerCell(el: any, index: number) {
  if (el) cells[index] = el as HTMLElement;
}

async function load(folder: string, selectPath = '') {
  isLoading.value = true;
  loadError.value = '';
  cells.length = 0;

  let result: BrowseListing;
  try {
    result = await browseFolder(folder, sort.value, descending.value);
  } catch (error) {
    console.error('Failed to list the folder:', error);
    loadError.value = String((error as any)?.message || error || '');
    isLoading.value = false;
    currentFolder.value = folder;
    listing.value = null;
    return;
  }
  isLoading.value = false;

  listing.value = result;
  currentFolder.value = result.folder;

  const index = selectPath ? result.files.findIndex((f) => samePath(f.path, selectPath)) : -1;
  selected.value = index >= 0 ? index : (result.files.length ? 0 : -1);
  await nextTick();
  gridRef.value?.focus();
  scrollSelectedIntoView();
  return index;
}

/** Windows hands back either separator and either case; compare forgivingly. */
function samePath(a: string, b: string) {
  const normalise = (value: string) => value.replace(/\\/g, '/').toLowerCase();
  return normalise(a) === normalise(b);
}

function scrollSelectedIntoView() {
  const cell = cells[selected.value];
  cell?.scrollIntoView({ block: 'nearest' });
}

async function reload() {
  const keep = files.value[selected.value]?.path || '';
  await load(currentFolder.value, keep);
}

async function toggleOrder() {
  descending.value = !descending.value;
  await reload();
}

async function goTo(folder: string) {
  viewerIndex.value = -1;
  await load(folder);
}

async function goUp() {
  const parent = listing.value?.parent;
  if (parent) await goTo(parent);
}

function openViewer(index: number) {
  if (index < 0 || index >= files.value.length) return;
  selected.value = index;
  viewerIndex.value = index;
}

function closeViewer() {
  viewerIndex.value = -1;
  nextTick(() => {
    gridRef.value?.focus();
    scrollSelectedIntoView();
  });
}

function step(delta: number) {
  const next = viewerIndex.value + delta;
  if (next < 0 || next >= files.value.length) return;
  viewerIndex.value = next;
  selected.value = next;
}

function onViewerWheel(event: WheelEvent) {
  step(event.deltaY > 0 ? 1 : -1);
}

/** A thumbnail that will not generate leaves an empty cell rather than a broken icon. */
function onThumbError(event: Event) {
  (event.target as HTMLImageElement).style.visibility = 'hidden';
}

function columnCount() {
  const grid = cells[0]?.parentElement;
  if (!grid) return 1;
  const style = getComputedStyle(grid);
  return Math.max(1, style.gridTemplateColumns.split(' ').filter(Boolean).length);
}

function onGridKeydown(event: KeyboardEvent) {
  if (viewerIndex.value >= 0) return;
  const total = files.value.length;
  if (!total) return;

  const columns = columnCount();
  let next = selected.value;
  switch (event.key) {
    case 'ArrowRight': next += 1; break;
    case 'ArrowLeft': next -= 1; break;
    case 'ArrowDown': next += columns; break;
    case 'ArrowUp': next -= columns; break;
    case 'Home': next = 0; break;
    case 'End': next = total - 1; break;
    case 'Enter': openViewer(selected.value); event.preventDefault(); return;
    case 'Backspace': void goUp(); event.preventDefault(); return;
    default: return;
  }
  event.preventDefault();
  selected.value = Math.min(total - 1, Math.max(0, next));
  scrollSelectedIntoView();
}

function onWindowKeydown(event: KeyboardEvent) {
  if (viewerIndex.value < 0) return;
  switch (event.key) {
    case 'Escape': closeViewer(); break;
    case 'ArrowRight': case 'PageDown': case ' ': step(1); break;
    case 'ArrowLeft': case 'PageUp': step(-1); break;
    case 'Home': viewerIndex.value = 0; selected.value = 0; break;
    case 'End':
      viewerIndex.value = files.value.length - 1;
      selected.value = viewerIndex.value;
      break;
    default: return;
  }
  event.preventDefault();
}

/**
 * The deliberate way into the library, for a folder worth keeping.
 *
 * The main window is told to refresh rather than being driven from here; it
 * owns the album list and the indexing queue.
 */
async function addFolderToLibrary() {
  if (isAdding.value || !currentFolder.value) return;
  isAdding.value = true;
  try {
    const album = await addAlbum(currentFolder.value);
    if (!album) throw new Error('add_album returned nothing');
    await tauriEmit('albums-refreshed');
    await tauriEmit('library-total-refreshed');
    await tauriEmit('browse-album-added', { albumId: album.id, path: currentFolder.value });
    toast.success(t('browse.added', { folder: folderName.value }));
  } catch (error) {
    console.error('Failed to add the browsed folder:', error);
    toast.error(t('browse.add_failed'));
  } finally {
    isAdding.value = false;
  }
}

onMounted(async () => {
  window.addEventListener('keydown', onWindowKeydown);

  unlistenGoto = await listen('browse-goto', async (event: any) => {
    viewerIndex.value = -1;
    await load(event?.payload?.folder || '', event?.payload?.select || '');
  });

  const query = router.currentRoute.value.query;
  const folder = String(query.folder || '');
  const select = String(query.select || '');
  if (!folder) {
    isLoading.value = false;
    return;
  }
  const index = await load(folder, select);
  // Launched on a specific picture: show it, do not make them click again.
  if (select && typeof index === 'number' && index >= 0) openViewer(index);
});

onUnmounted(() => {
  window.removeEventListener('keydown', onWindowKeydown);
  unlistenGoto?.();
  unlistenGoto = null;
});
</script>

<style scoped>
/* Let the browser skip layout and paint for rows that are scrolled away, so a
   folder with a few thousand pictures stays responsive without windowing. */
.cell {
  content-visibility: auto;
  contain-intrinsic-size: 170px;
}
</style>
