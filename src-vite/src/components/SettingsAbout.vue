<template>
  <div class="flex flex-col items-start justify-start gap-4 h-full text-base-content/70 cursor-default">

    <!-- logo -->
    <div class="px-2 flex w-full flex-row items-center justify-start gap-4">
      <div class="shrink-0">
        <img :src="iconLogo" class="w-20 h-20 select-none [-webkit-app-region:no-drag]" draggable="false" />
      </div>
      <div class="flex flex-col text-left">
        <h3 class="text-xl">猫叔的图 <span class="text-xs opacity-50 ml-1">{{ packageInfo.name }} v{{ packageInfo.version }}</span></h3>
        <p class="mt-2 text-sm">{{ $t('settings.about.package.app_description') }}</p>
      </div>
    </div>

    <!-- package info -->
    <div class="w-full max-w-lg rounded-box border border-base-content/5 bg-base-300/30 p-4 shadow-sm">
      <div class="space-y-3 text-left">
        <div class="grid grid-cols-[84px_1fr] items-start gap-3 text-sm">
          <div class="text-base-content/30">
            {{ $t('settings.about.package.version') }}
          </div>
          <div class="flex items-center gap-2">
            <span>{{ displayVersion }}</span>
            <button
              class="badge badge-sm border-0 px-2 py-2 font-medium transition-colors hover:text-primary"
              :class="isUpdateActionEnabled ? 'badge-primary cursor-pointer' : 'badge-neutral/60 cursor-pointer'"
              :disabled="isInstallingUpdate || isCheckingUpdate"
              :title="updateButtonTooltip"
              @click="handleUpdateAction"
            >
              <span v-if="isInstallingUpdate || isCheckingUpdate" class="loading loading-spinner loading-xs"></span>
              <span>{{ updateButtonText }}</span>
            </button>
          </div>
        </div>

        <div class="grid grid-cols-[84px_1fr] items-start gap-3 text-sm">
          <div class="text-base-content/30">
            {{ $t('settings.about.package.build_time') }}
          </div>
          <div>{{ buildTime }}</div>
        </div>

        <div class="grid grid-cols-[84px_1fr] items-start gap-3 text-sm">
          <div class="text-base-content/30">
            {{ $t('settings.about.package.license') }}
          </div>
          <div>{{ packageInfo.license }}</div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { getPackageInfo, getBuildTime } from '@/common/api';
import { useAppUpdater } from '@/common/updater';
import iconLogo from '@/assets/images/icon.png';

const packageInfo = ref<any>({
  name: '',
  description: '',
  version: '',
  commit_hash: '',
  license: '',
  authors: [],
  homepage: '',
  repository: ''
});
const buildTime = ref('');
const displayVersion = computed(() => {
  const version = packageInfo.value.version || '';
  const commitHash = packageInfo.value.commit_hash || packageInfo.value.commitHash || '';
  return commitHash ? `${version} (${commitHash})` : version;
});
const { locale, messages } = useI18n();
const localeMsg = computed(() => messages.value[locale.value] as any);
const {
  isCheckingUpdate,
  isInstallingUpdate,
  updateButtonTooltip,
  updateButtonText,
  isUpdateActionEnabled,
  handleUpdateAction,
} = useAppUpdater(localeMsg, { toastPlacement: 'center' });

onMounted(async () => {
  try {
    packageInfo.value = await getPackageInfo();
    const time = await getBuildTime();
    buildTime.value = time || '';
  } catch (error) {
    console.error('Failed to load about info:', error);
  }
});
</script>
