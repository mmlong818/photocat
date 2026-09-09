<template>
  <ModalDialog :title="$t('tag.edit_tag')" :width="600" @cancel="clickCancel">
    <section class="space-y-3">
      <div class="flex items-center gap-2">
        <div 
          :class="[
            'grow h-8 flex items-center rounded-box overflow-hidden transition-colors bg-base-100',
            isSearchFocused ? 'border-2 border-primary' : 'border border-neutral-content/30 hover:border-neutral-content/70'
          ]"
        >
          <IconSearch class="ml-2 w-4 h-4 text-base-content/70" />
          <input
            ref="tagSearchInputRef"
            type="text"
            v-model="tagSearch"
            :placeholder="$t('tag.search_tags')"
            class="w-full bg-transparent border-none focus:ring-0 px-2 text-sm placeholder-base-content/30 focus:outline-none"
            @focus="onSearchFocus"
            @blur="onSearchBlur"
          />
          <button
            v-if="tagSearch"
            type="button"
            class="mr-1 p-1 rounded-box text-base-content/30 hover:text-base-content/70"
            @click="tagSearch = ''; tagSearchInputRef?.focus()"
          >
            <IconClose class="w-4 h-4" />
          </button>
        </div>
        <div
          :class="[
            'w-1/2 h-8 flex items-center rounded-box overflow-hidden transition-colors bg-base-100',
            isNewTagFocused ? 'border-2 border-primary' : 'border border-neutral-content/30 hover:border-neutral-content/70'
          ]"
        >
          <input
            ref="newTagNameInputRef"
            type="text"
            v-model="newTagName"
            :placeholder="$t('tag.enter_new_tag_name')"
            class="w-full bg-transparent border-none focus:ring-0 px-2 text-sm placeholder-base-content/30 focus:outline-none"
            @focus="isNewTagFocused = true"
            @blur="isNewTagFocused = false"
            @keydown.enter="addNewTag"
          />
          <button
            v-if="newTagName"
            type="button"
            class="mr-1 p-1 rounded-box text-base-content/30 hover:text-base-content/70"
            @click="newTagName = ''; newTagNameInputRef?.focus()"
          >
            <IconClose class="w-4 h-4" />
          </button>
        </div>
        <TButton 
          :icon="IconAdd"
          :tooltip="$t('msgbox.new_tag.title')"
          @click="addNewTag"
        />
      </div>

      <div v-if="allTags.length" class="text-[10px] uppercase tracking-widest font-bold text-base-content/30 select-none">{{ $t('tag.title') }} ({{ allTags.length }})</div>
      <div class="min-h-24 max-h-52 overflow-y-auto rounded-box p-1 bg-base-100/30 border border-base-content/5 flex" :class="filteredTags.length === 0 ? 'items-center justify-center' : ''">
        <div v-if="filteredTags.length > 0" class="w-full">
          <div
            v-for="(tag, index) in filteredTags"
            :key="tag.id"
            :class="[
              'group w-full p-2 flex items-center gap-2 rounded-box text-left cursor-pointer transition-colors',
              {
                'text-primary': selectedTags.has(tag.id),
                'bg-base-content/5': intermediateTags.has(tag.id) && !selectedTags.has(tag.id),
                'hover:bg-base-content/5': !selectedTags.has(tag.id) && !intermediateTags.has(tag.id),
                'ring-2 ring-primary ring-offset-1 ring-offset-base-100': focusedTagIndex === index,
              }
            ]"
            @click="toggleTag(tag.id)"
          >
            <label class="flex items-center cursor-pointer shrink-0" @click.stop @dblclick.stop>
              <input
                type="checkbox"
                class="checkbox checkbox-xs"
                :class="selectedTags.has(tag.id) ? 'checkbox-primary opacity-70' : ''"
                :checked="selectedTags.has(tag.id)"
                :indeterminate="intermediateTags.has(tag.id)"
                @change="toggleTag(tag.id)"
              />
            </label>
            <IconTag class="w-4 h-4 shrink-0" />
            <input
              v-if="renamingId === Number(tag.id)"
              ref="renameInput"
              v-model="renameValue"
              class="input px-1 min-w-0 flex-1 text-sm"
              maxlength="255"
              @click.stop
              @keydown.stop
              @keydown.enter.prevent="commitRename(tag)"
              @keydown.escape.prevent="cancelRename"
              @blur="commitRename(tag)"
            />
            <span v-else class="min-w-0 flex-1 truncate">{{ tag.name }}</span>
            <span v-if="renamingId !== Number(tag.id) && Number(tag.count || 0) > 0" class="sidebar-item-count shrink-0 group-hover:hidden">{{ Number(tag.count || 0).toLocaleString() }}</span>
            <div v-if="renamingId !== Number(tag.id)" class="shrink-0 hidden group-hover:flex">
              <button type="button" class="p-1 text-base-content/40 hover:text-base-content cursor-pointer" :title="$t('menu.tag.rename')" @click.stop="startRename(tag)"><IconEdit class="w-4 h-4" /></button>
              <button type="button" class="p-1 text-base-content/40 hover:text-error cursor-pointer" :title="$t('tag.delete_tag')" @click.stop="deleteTarget = tag"><IconTrash class="w-4 h-4" /></button>
            </div>
          </div>
        </div>
        <span v-else class="text-base-content/30">{{ $t('tag.not_found') }}</span>
      </div>
      <div v-if="tagLoadFailed" class="text-sm text-error">
        {{ $t('tag.load_failed') }}
      </div>
    </section>

    <!-- cancel and OK buttons -->
    <div class="mt-4 flex justify-end space-x-4">
      <button 
        class="t-button-default" 
        @click="clickCancel"
      >{{ $t('msgbox.cancel') }}</button>
      
      <button 
        class="t-button-primary" 
        :disabled="isLoadingTags || isApplyingTags || tagLoadFailed"
        @click="clickOk"
      >{{ $t('msgbox.ok') }}</button>

    </div>
  </ModalDialog>
  <MessageBox
    v-if="deleteTarget"
    :title="$t('msgbox.delete_tag.title')"
    :message="$t('msgbox.delete_tag.content', { tag: deleteTarget.name })"
    :OkText="$t('msgbox.delete_tag.ok')"
    :cancelText="$t('msgbox.cancel')"
    :warningOk="true"
    @ok="confirmDelete"
    @cancel="deleteTarget = null"
  />
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onBeforeUnmount } from 'vue';
import { emit as tauriEmit } from '@tauri-apps/api/event';
import { useI18n } from 'vue-i18n';
import { useToast } from '@/common/toast';
import { 
  getAllTags, 
  createTag, 
  getTagSelectionCounts,
  applyTagsToFiles,
  deleteTag,
  renameTag,
} from '@/common/api';
import { IconAdd, IconClose, IconEdit, IconSearch, IconTag, IconTrash } from '@/common/icons';
import MessageBox from './MessageBox.vue';
import TButton from './TButton.vue';
import { libConfig } from '@/common/config';
import { useUIStore } from '@/stores/uiStore';
import ModalDialog from '@/components/ModalDialog.vue';

const props = defineProps({
  fileIds: {
    type: Array as () => number[],
    default: () => [],
  },
});

const emit = defineEmits(['ok', 'cancel', 'states-changed']);
const uiStore = useUIStore();
const toast = useToast();
const { t } = useI18n();

const allTags = ref<any[]>([]);
const tagSearchInputRef = ref<HTMLInputElement | null>(null);
const newTagNameInputRef = ref<HTMLInputElement | null>(null);
const tagSearch = ref('');
const newTagName = ref('');
const isSearchFocused = ref(false);
const isNewTagFocused = ref(false);
const focusedTagIndex = ref(-1); // -1 = no tag focused
const isInTagList = ref(false); // true = keyboard focus is in tag list
const isLoadingTags = ref(false);
const isApplyingTags = ref(false);
const tagLoadFailed = ref(false);
const renamingId = ref<number | null>(null);
const renameValue = ref('');
const renameInput = ref<HTMLInputElement | HTMLInputElement[] | null>(null);
const deleteTarget = ref<any | null>(null);

// Sets to track tag states
const selectedTags = ref<Set<number>>(new Set()); // Tags present on ALL selected files
const intermediateTags = ref<Set<number>>(new Set()); // Tags present on SOME selected files
const initialSelectedTags = ref<Set<number>>(new Set());
const initialIntermediateTags = ref<Set<number>>(new Set());
const tagChanges = ref<Map<number, 'add' | 'remove'>>(new Map());

const filteredTags = computed(() => {
  if (!tagSearch.value) {
    return allTags.value;
  }
  return allTags.value.filter(tag =>
    tag.name.toLowerCase().includes(tagSearch.value.toLowerCase())
  );
});

onMounted(async () => {
  window.addEventListener('keydown', handleKeyDown);
  uiStore.pushInputHandler('TaggingDialog');

  await nextTick();
  tagSearchInputRef.value?.focus();

  loadAllTags();
  loadExistingTagsForFiles();
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeyDown);
  uiStore.removeInputHandler('TaggingDialog');
});

// load all tags
async function loadAllTags() {
  const tags = await getAllTags(0);
  const sidebarCounts = libConfig.tag.counts || {};
  allTags.value = (tags || []).map((tag: any) => ({
    ...tag,
    count: Number(sidebarCounts[String(tag.id)] || 0),
  }));
}

async function loadExistingTagsForFiles() {
  selectedTags.value.clear();
  intermediateTags.value.clear();
  initialSelectedTags.value.clear();
  initialIntermediateTags.value.clear();
  tagChanges.value.clear();
  tagLoadFailed.value = false;

  if (props.fileIds.length === 0) {
    return;
  }

  isLoadingTags.value = true;
  try {
    const counts = await getTagSelectionCounts(props.fileIds);
    if (counts === null) {
      tagLoadFailed.value = true;
      return;
    }
    for (const entry of counts) {
      const tagId = Number(entry.tag_id);
      const count = Number(entry.count);
      if (count === props.fileIds.length) {
        selectedTags.value.add(tagId);
        initialSelectedTags.value.add(tagId);
      } else if (count > 0) {
        intermediateTags.value.add(tagId);
        initialIntermediateTags.value.add(tagId);
      }
    }
  } finally {
    isLoadingTags.value = false;
  }
}

async function addNewTag() {
  const trimmedName = newTagName.value.trim();
  if (trimmedName) {
    const existingTag = allTags.value.find(tag => tag.name.toLowerCase() === trimmedName.toLowerCase());
    if (existingTag) {
      toast.error(t('tag.name_exists'));
      return;
    } else {
      const newTag = await createTag(trimmedName);
      if (newTag) {
        allTags.value.push(newTag);
        toggleTag(newTag.id);
        await tauriEmit('tags-changed');
      } else {
        toast.error(t('tag.name_save_failed'));
      }
    }
    newTagName.value = ''; // Clear input
  }
}

function toggleTag(tagId: number) {
  const normalized = Number(tagId);
  const next = new Map(tagChanges.value);
  const current = next.get(normalized);
  const initialState = initialSelectedTags.value.has(normalized)
    ? 'selected'
    : initialIntermediateTags.value.has(normalized)
      ? 'intermediate'
      : 'none';

  if (current === 'add') {
    if (initialState === 'none') {
      next.delete(normalized);
    } else {
      next.set(normalized, 'remove');
    }
    setTagVisualState(normalized, 'none');
  } else if (current === 'remove') {
    next.delete(normalized);
    setTagVisualState(normalized, initialState);
  } else if (selectedTags.value.has(normalized)) {
    next.set(normalized, 'remove');
    setTagVisualState(normalized, 'none');
  } else {
    next.set(normalized, 'add');
    setTagVisualState(normalized, 'selected');
  }
  tagChanges.value = next;
}

function setTagVisualState(tagId: number, state: 'selected' | 'intermediate' | 'none') {
  const selected = new Set(selectedTags.value);
  const intermediate = new Set(intermediateTags.value);
  selected.delete(tagId);
  intermediate.delete(tagId);
  if (state === 'selected') selected.add(tagId);
  if (state === 'intermediate') intermediate.add(tagId);
  selectedTags.value = selected;
  intermediateTags.value = intermediate;
}

async function startRename(tag: any) {
  renamingId.value = Number(tag.id);
  renameValue.value = String(tag.name || '');
  await nextTick();
  const input = Array.isArray(renameInput.value) ? renameInput.value[0] : renameInput.value;
  input?.focus({ preventScroll: true });
  input?.select();
}

function cancelRename() {
  renamingId.value = null;
  renameValue.value = '';
}

async function commitRename(tag: any) {
  if (renamingId.value !== Number(tag.id)) return;
  const name = renameValue.value.trim();
  if (name && name !== tag.name) {
    const duplicate = allTags.value.some(item =>
      Number(item.id) !== Number(tag.id)
      && String(item.name).toLocaleLowerCase() === name.toLocaleLowerCase()
    );
    if (duplicate) {
      toast.error(t('tag.name_exists'));
      cancelRename();
      return;
    }
    if (await renameTag(Number(tag.id), name)) {
      tag.name = name;
      await tauriEmit('tags-changed');
    } else {
      toast.error(t('tag.name_save_failed'));
    }
  }
  cancelRename();
}

async function confirmDelete() {
  const tag = deleteTarget.value;
  if (!tag) return;
  deleteTarget.value = null;
  if (!await deleteTag(Number(tag.id))) return;
  const tagId = Number(tag.id);
  allTags.value = allTags.value.filter(item => Number(item.id) !== tagId);
  selectedTags.value.delete(tagId);
  intermediateTags.value.delete(tagId);
  initialSelectedTags.value.delete(tagId);
  initialIntermediateTags.value.delete(tagId);
  tagChanges.value.delete(tagId);
  const states = await applyTagsToFiles(props.fileIds, [], []);
  if (states !== null) emit('states-changed', states);
  await tauriEmit('tags-changed');
}

async function clickOk() {
  if (isLoadingTags.value || isApplyingTags.value || tagLoadFailed.value) return;
  isApplyingTags.value = true;
  const addTagIds = Array.from(tagChanges.value)
    .filter(([, change]) => change === 'add')
    .map(([tagId]) => tagId);
  const removeTagIds = Array.from(tagChanges.value)
    .filter(([, change]) => change === 'remove')
    .map(([tagId]) => tagId);
  const result = await applyTagsToFiles(props.fileIds, addTagIds, removeTagIds);
  if (result !== null) {
    await tauriEmit('tags-changed');
    emit('ok', result);
  } else {
    isApplyingTags.value = false;
  }
}

function clickCancel() {
  emit('cancel');
}

// Reset tag focus when search results change
watch(filteredTags, () => {
  focusedTagIndex.value = -1;
  isInTagList.value = false;
});

function onSearchFocus() {
  isSearchFocused.value = true;
  isInTagList.value = false;
  focusedTagIndex.value = -1;
}

function onSearchBlur() {
  isSearchFocused.value = false;
}

function enterTagList() {
  if (filteredTags.value.length > 0) {
    isInTagList.value = true;
    focusedTagIndex.value = 0;
    tagSearchInputRef.value?.blur();
    newTagNameInputRef.value?.blur();
  }
}

function exitTagList() {
  isInTagList.value = false;
  focusedTagIndex.value = -1;
  tagSearchInputRef.value?.focus();
}

// Keyboard: ArrowDown→tag list, Space→toggle, Enter→OK, Escape→back/close
const handleKeyDown = (e: KeyboardEvent) => {
  if (!uiStore.isInputActive('TaggingDialog')) return;

  const { key } = e;
  const active = document.activeElement;
  const isInAnyInput = active === tagSearchInputRef.value || active === newTagNameInputRef.value;

  if (key === 'Tab' && active === newTagNameInputRef.value && !e.shiftKey) {
    e.preventDefault();
    if (filteredTags.value.length > 0) {
      enterTagList();
    } else {
      tagSearchInputRef.value?.focus();
    }
    return;
  }

  // Escape: tag list → search input → close dialog
  if (key === 'Escape') {
    if (isInTagList.value) {
      exitTagList();
    } else {
      clickCancel();
    }
    return;
  }

  // ArrowDown → enter tag list (from any state, unless already navigating)
  if (key === 'ArrowDown' && !isInTagList.value) {
    e.preventDefault();
    enterTagList();
    return;
  }

  // Tag list keyboard navigation
  if (isInTagList.value && filteredTags.value.length > 0) {
    const lastIndex = filteredTags.value.length - 1;

    if (key === 'ArrowRight') {
      e.preventDefault();
      focusedTagIndex.value = focusedTagIndex.value >= lastIndex ? 0 : focusedTagIndex.value + 1;
    } else if (key === 'ArrowLeft') {
      e.preventDefault();
      focusedTagIndex.value = focusedTagIndex.value <= 0 ? lastIndex : focusedTagIndex.value - 1;
    } else if (key === 'ArrowUp') {
      e.preventDefault();
      exitTagList();
    } else if (key === ' ') {
      e.preventDefault();
      const tag = filteredTags.value[focusedTagIndex.value];
      if (tag) toggleTag(tag.id);
    } else if (key === 'Enter') {
      e.preventDefault();
      clickOk();
    } else if (key === 'Tab') {
      e.preventDefault();
      if (e.shiftKey) {
        isInTagList.value = false;
        focusedTagIndex.value = -1;
        newTagNameInputRef.value?.focus();
      } else {
        exitTagList(); // Tab → back to search input (keep focus inside dialog)
      }
    }
    return;
  }

  // Enter → confirm dialog (when not typing in any input)
  if (key === 'Enter' && !isInAnyInput) {
    e.preventDefault();
    clickOk();
  }
};
</script>
