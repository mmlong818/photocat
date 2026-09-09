<template>
  <div v-if="!isReady" class="w-screen h-screen flex flex-col items-center justify-center gap-4 bg-base-300">
    <span class="loading loading-spinner loading-lg text-primary"></span>
    <p class="text-sm text-base-content/60">{{ startupMessage }}</p>
  </div>
  <template v-else>
    <router-view />
    <ToastContainer />
  </template>
</template>
 
<script setup>
import { ref, computed, watch, onMounted, onUnmounted } from 'vue';
import { emit, listen } from '@tauri-apps/api/event';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { useI18n } from 'vue-i18n';
import { useConfigStore } from '@/stores/configStore';
import { useLibraryStore } from '@/stores/libraryStore';
import { clearIndexRecoveryInfo, getModelStatus } from '@/common/api';
import { isMac, setTheme, SCALE_VALUES } from '@/common/utils';
import { matchesShortcut } from '@/common/shortcuts';
import ToastContainer from '@/components/ToastContainer.vue';

const { t } = useI18n();
const libConfig = useLibraryStore();
const isReady = ref(false);
const config = useConfigStore();
// What the startup screen is currently waiting for. The image-text search
// models take the longest, so they are named explicitly rather than leaving a
// bare spinner on screen.
const startupStep = ref('models');
const startupMessage = computed(() =>
  startupStep.value === 'models' ? t('startup.loading_models') : t('startup.loading_library')
);
let unlistenMainCloseRequested = null;
let unlistenModelStatus = null;
let isHandlingMainClose = false;

// Resolves once the models are loaded, or once loading them has failed.
//
// The backend emits `model-status` when it finishes, but it can finish before
// this window is listening, so the current value is also read directly.
const waitForModels = async () => {
  let settle;
  const settled = new Promise((resolve) => {
    settle = resolve;
  });

  unlistenModelStatus = await listen('model-status', (event) => {
    if (event.payload !== 'loading') settle(event.payload);
  });

  const current = await getModelStatus();
  if (current !== 'loading') settle(current);

  const status = await settled;
  if (status === 'failed') {
    console.warn('[App] image-text search unavailable; continuing without it');
  }
};

// Auto-save library state when any config changes
watch(() => libConfig.$state, () => {
  if (libConfig._initialized) {
    libConfig.save();
  }
}, { deep: true });

watch(
  () => Number(config.settings.scale || 1),
  (newScale) => {
    const win = getCurrentWebviewWindow();
    if (win.label === 'main') {
      applyMainWindowScale(newScale);
    }
  }
);

onMounted(async () => {
  const win = getCurrentWebviewWindow();
  if (win.label === 'main') {
    window.addEventListener('keydown', handleKeyDown, { capture: true });
    applyMainWindowScale(Number(config.settings.scale || 1));
    if (import.meta.env.PROD) {
      window.addEventListener('contextmenu', handleContextMenu);
    }
    if (typeof win.onCloseRequested === 'function') {
      unlistenMainCloseRequested = await win.onCloseRequested(async (event) => {
        if (isHandlingMainClose) return;
        isHandlingMainClose = true;

        try {
          event.preventDefault();

          if (isMac) {
            // macOS convention: close hides the window; the app stays alive and
            // can be reopened from the Dock. Quitting still uses the native menu.
            await win.hide();
            return;
          }

          try {
            if (libConfig._initialized) {
              // Mark scanning as paused so it won't auto-resume on restart
              if (libConfig.index.status === 1) {
                libConfig.index.status = 2;
              }
              // Normal close → clear recovery trace (crash leaves it intact)
              await clearIndexRecoveryInfo();
              await libConfig.save();
            }
          } finally {
            await win.close();
          }
        } finally {
          isHandlingMainClose = false;
        }
      });
    } else {
      unlistenMainCloseRequested = await win.listen('tauri://close-requested', async () => {
        if (isMac) {
          await win.hide();
        } else if (libConfig._initialized) {
          // Mark scanning as paused so it won't auto-resume on restart
          if (libConfig.index.status === 1) {
            libConfig.index.status = 2;
          }
          await clearIndexRecoveryInfo();
          await libConfig.save();
        }
      });
    }
  }

  setTheme(config.settings.appearance, 
    config.settings.appearance === 0 ? config.settings.lightTheme : config.settings.darkTheme);

  // Show the window before any of the slow work so launching the app has an
  // immediate, visible response instead of several seconds of nothing.
  // Secondary windows open after loading has already finished, so only the
  // main window waits on the models.
  if (win.label === 'main') {
    await win.show();
    await waitForModels();
  }

  startupStep.value = 'library';

  // Initialize library state from backend
  try {
    await libConfig.init();
  } catch (error) {
    console.error('[App] Library initialization failed:', error);
    // Continue anyway - user can retry from UI
  } finally {
    isReady.value = true;
  }
});

onUnmounted(async () => {
  const win = getCurrentWebviewWindow();
  if (win.label === 'main') {
    window.removeEventListener('keydown', handleKeyDown, { capture: true });
    document.documentElement.style.fontSize = '';
    if (import.meta.env.PROD) {
      window.removeEventListener('contextmenu', handleContextMenu);
    }
  }
  unlistenMainCloseRequested?.();
  unlistenMainCloseRequested = null;
  unlistenModelStatus?.();
  unlistenModelStatus = null;
});

const handleKeyDown = (event) => {
  if (handleMainWindowScaleShortcut(event)) {
    return;
  }

  if (!isMac && matchesShortcut('app.preferences', event)) {
    event.preventDefault();
    event.stopPropagation();
    emit('app-open-preferences');
    return;
  }

  emit('global-keydown', {
    key: event.key,
    code: event.code,
    altKey: event.altKey,
    ctrlKey: event.ctrlKey,
    metaKey: event.metaKey,
    shiftKey: event.shiftKey,
  });
};

function normalizeScale(value) {
  return SCALE_VALUES.find((item) => item === Number(value)) ?? 1;
}

function applyMainWindowScale(scale) {
  const normalizedScale = normalizeScale(scale);
  document.documentElement.style.fontSize = `${normalizedScale * 16}px`;
}

function handleMainWindowScaleShortcut(event) {
  const win = getCurrentWebviewWindow();
  if (win.label !== 'main') return false;

  const isScaleUp = matchesShortcut('app.scale.increase', event);
  const isScaleDown = matchesShortcut('app.scale.decrease', event);
  const isScaleReset = matchesShortcut('app.scale.reset', event);

  if (!isScaleUp && !isScaleDown && !isScaleReset) return false;

  event.preventDefault();
  event.stopPropagation();

  const currentScale = normalizeScale(config.settings.scale || 1);
  const currentIndex = SCALE_VALUES.indexOf(currentScale);
  let nextScale = currentScale;

  if (isScaleReset) {
    nextScale = 1;
  } else if (isScaleUp) {
    nextScale = SCALE_VALUES[Math.min(currentIndex + 1, SCALE_VALUES.length - 1)];
  } else if (isScaleDown) {
    nextScale = SCALE_VALUES[Math.max(currentIndex - 1, 0)];
  }

  if (nextScale !== currentScale) {
    config.setScale(nextScale);
  }
  applyMainWindowScale(nextScale);
  return true;
}

const handleContextMenu = (e) => {
  e.preventDefault();
};

</script>

