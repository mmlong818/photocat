<template>
  <ModalDialog :title="title" :width="440" @cancel="$emit('cancel')">
    <section class="space-y-3">
      <p v-if="message" class="text-sm text-base-content/70">{{ message }}</p>

      <div class="h-8 flex items-center rounded-box overflow-hidden bg-base-100 border border-neutral-content/30 focus-within:border-primary">
        <IconSearch class="ml-2 w-4 h-4 text-base-content/70 shrink-0" />
        <input
          ref="searchInput"
          v-model="query"
          :placeholder="$t('person.picker.search')"
          class="w-full bg-transparent border-none focus:ring-0 px-2 text-sm placeholder-base-content/30 focus:outline-none"
        />
        <button v-if="query" class="mr-1 p-1 text-base-content/30 hover:text-base-content/70" @click="query = ''">
          <IconClose class="w-4 h-4" />
        </button>
      </div>

      <div class="min-h-32 max-h-64 overflow-y-auto rounded-box p-1 bg-base-100/30 border border-base-content/5">
        <p v-if="visible.length === 0" class="p-4 text-center text-sm text-base-content/30">
          {{ $t('person.picker.empty') }}
        </p>
        <button
          v-for="person in visible"
          :key="person.id"
          type="button"
          class="w-full p-2 flex items-center gap-2 rounded-box text-left transition-colors"
          :class="chosenId === person.id ? 'bg-primary/10 text-primary' : 'hover:bg-base-content/5'"
          @click="chosenId = person.id"
          @dblclick="confirm"
        >
          <div class="w-8 h-8 rounded-full overflow-hidden bg-base-300/70 shrink-0 flex items-center justify-center">
            <img
              v-if="person.thumbnail"
              :src="'data:image/jpeg;base64,' + person.thumbnail"
              class="w-full h-full object-cover"
            />
            <IconPerson v-else class="w-5 h-5 text-base-content/30" />
          </div>
          <span class="flex-1 truncate text-sm">{{ displayName(person) }}</span>
          <span v-if="person.count" class="text-[10px] tabular-nums text-base-content/30">
            {{ person.count.toLocaleString() }}
          </span>
        </button>
      </div>
    </section>

    <div class="mt-5 flex justify-end gap-3">
      <button class="t-button-default" @click="$emit('cancel')">{{ $t('msgbox.cancel') }}</button>
      <button class="t-button-primary" :disabled="!chosenId" @click="confirm">{{ confirmText }}</button>
    </div>
  </ModalDialog>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import ModalDialog from '@/components/ModalDialog.vue';
import { IconClose, IconPerson, IconSearch } from '@/common/icons';
import { useUIStore } from '@/stores/uiStore';
import { getPersonsPage } from '@/common/api';

const props = defineProps<{
  title: string;
  message?: string;
  confirmText: string;
  /// People to leave out — normally the one being merged or reassigned from.
  excludeIds?: number[];
}>();
const emit = defineEmits<{ (e: 'pick', id: number): void; (e: 'cancel'): void }>();

const { t } = useI18n();
const uiStore = useUIStore();

const persons = ref<any[]>([]);
const query = ref('');
const chosenId = ref<number | null>(null);
const searchInput = ref<HTMLInputElement | null>(null);

function displayName(person: any) {
  return person.name || t('menu.person.unnamed');
}

const visible = computed(() => {
  const excluded = new Set(props.excludeIds ?? []);
  const needle = query.value.trim().toLowerCase();
  return persons.value.filter((person) => {
    if (excluded.has(Number(person.id))) return false;
    if (!needle) return true;
    return displayName(person).toLowerCase().includes(needle);
  });
});

function confirm() {
  if (chosenId.value) emit('pick', chosenId.value);
}

onMounted(async () => {
  uiStore.pushInputHandler('PersonPickerDialog');
  // One page is plenty: the list is searchable and people are few.
  const page = await getPersonsPage({
    sort: 0,
    offset: 0,
    limit: 500,
    search: '',
    smallFileFilter: 0,
    refreshSummary: null,
  });
  persons.value = Array.isArray(page?.persons) ? page.persons : [];
  await nextTick();
  searchInput.value?.focus();
});

onBeforeUnmount(() => uiStore.removeInputHandler('PersonPickerDialog'));
</script>
