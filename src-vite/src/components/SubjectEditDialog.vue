<template>
  <ModalDialog :title="dialogTitle" :width="520" @cancel="$emit('cancel')">
    <section class="space-y-4">
      <!-- name -->
      <div class="space-y-1">
        <label class="text-[10px] uppercase tracking-widest font-bold text-base-content/30 select-none">
          {{ $t('subject.custom.name') }}
        </label>
        <input
          ref="nameInput"
          v-model="name"
          :maxlength="SUBJECT_NAME_MAX"
          :placeholder="$t('subject.custom.name_placeholder')"
          class="w-full h-8 px-2 text-sm rounded-box bg-base-100 border border-neutral-content/30 focus:border-primary focus:outline-none"
          @keydown.enter.prevent="submit"
        />
      </div>

      <!-- prompt -->
      <div class="space-y-1">
        <label class="text-[10px] uppercase tracking-widest font-bold text-base-content/30 select-none">
          {{ $t('subject.custom.prompt') }}
        </label>
        <textarea
          v-model="prompt"
          rows="3"
          :maxlength="SUBJECT_PROMPT_MAX"
          :placeholder="$t('subject.custom.prompt_placeholder')"
          class="w-full px-2 py-1.5 text-sm rounded-box bg-base-100 border border-neutral-content/30 focus:border-primary focus:outline-none resize-none"
        ></textarea>
        <p class="text-xs text-base-content/50 leading-relaxed">
          {{ $t('subject.custom.prompt_hint') }}
        </p>
      </div>

      <!-- precision -->
      <div class="space-y-1">
        <label class="text-[10px] uppercase tracking-widest font-bold text-base-content/30 select-none">
          {{ $t('subject.custom.precision') }}
        </label>
        <div class="flex gap-2">
          <button
            v-for="option in precisionOptions"
            :key="option.value"
            type="button"
            class="flex-1 h-8 text-sm rounded-box border transition-colors"
            :class="precision === option.value
              ? 'border-primary text-primary bg-primary/10'
              : 'border-neutral-content/30 text-base-content/70 hover:bg-base-content/5'"
            @click="precision = option.value"
          >
            {{ option.label }}
          </button>
        </div>
        <p class="text-xs text-base-content/50 leading-relaxed">
          {{ $t('subject.custom.precision_hint') }}
        </p>
      </div>
    </section>

    <div class="mt-5 flex items-center justify-end gap-3">
      <button class="t-button-default" @click="$emit('cancel')">{{ $t('msgbox.cancel') }}</button>
      <button class="t-button-primary" :disabled="!canSave" @click="submit">{{ $t('msgbox.ok') }}</button>
    </div>
  </ModalDialog>
</template>

<script setup lang="ts">
import { computed, nextTick, onMounted, onBeforeUnmount, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import ModalDialog from '@/components/ModalDialog.vue';
import { useUIStore } from '@/stores/uiStore';
import {
  SUBJECT_NAME_MAX,
  SUBJECT_PROMPT_MAX,
  isValidSubject,
  type CustomSubject,
  type SubjectPrecision,
} from '@/common/customSubjects';

const props = defineProps<{ subject?: CustomSubject | null }>();
const emit = defineEmits<{
  (e: 'save', value: { name: string; prompt: string; precision: SubjectPrecision }): void;
  (e: 'cancel'): void;
}>();

const { t } = useI18n();
const uiStore = useUIStore();

const name = ref(props.subject?.name ?? '');
const prompt = ref(props.subject?.prompt ?? '');
const precision = ref<SubjectPrecision>(props.subject?.precision ?? 'standard');
const nameInput = ref<HTMLInputElement | null>(null);

const dialogTitle = computed(() =>
  props.subject ? t('subject.custom.edit_title') : t('subject.custom.add_title'),
);

const precisionOptions = computed<{ value: SubjectPrecision; label: string }[]>(() => [
  { value: 'strict', label: t('subject.custom.precision_strict') },
  { value: 'standard', label: t('subject.custom.precision_standard') },
  { value: 'loose', label: t('subject.custom.precision_loose') },
]);

const canSave = computed(() => isValidSubject(name.value, prompt.value));

function submit() {
  if (!canSave.value) return;
  emit('save', {
    name: name.value.trim(),
    prompt: prompt.value.trim(),
    precision: precision.value,
  });
}

onMounted(async () => {
  uiStore.pushInputHandler('SubjectEditDialog');
  await nextTick();
  nameInput.value?.focus();
});

onBeforeUnmount(() => {
  uiStore.removeInputHandler('SubjectEditDialog');
});
</script>
