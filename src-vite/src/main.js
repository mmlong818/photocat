import { createApp } from 'vue'
import { createI18n } from 'vue-i18n'
import { createPinia } from 'pinia'
import piniaPersistedState from 'pinia-plugin-persistedstate'
import { emit, listen } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import 'cally'
import router from '@/common/router'
import App from '@/App.vue'
import { useConfigStore } from '@/stores/configStore'
import '@/assets/app.css'

// I18n
import en from '@/locales/en.json'
import zh from '@/locales/zh.json'
import es from '@/locales/es.json'
import fr from '@/locales/fr.json'
import de from '@/locales/de.json'
import ja from '@/locales/ja.json'
import ko from '@/locales/ko.json'
import ru from '@/locales/ru.json'
import pt from '@/locales/pt.json'

// Create the app instance
const app = createApp(App)

// Create Pinia store and use the persisted state plugin
const pinia = createPinia()
pinia.use(piniaPersistedState)
app.use(pinia) // Use Pinia
const config = useConfigStore() // Use the config store
const currentWindowLabel = getCurrentWebviewWindow().label
const isMainWindow = currentWindowLabel === 'main'
const isSettingsWindow = currentWindowLabel === 'settings'

if (isMainWindow) {
  config.$subscribe((_mutation, state) => {
    void emit('config-settings-synced', JSON.parse(JSON.stringify(state.settings)))
  })
} else if (!isSettingsWindow) {
  listen('config-settings-synced', (event) => {
    Object.assign(config.settings, event.payload)
  })
}

// Create the I18n instance
const i18n = createI18n({
  legacy: false, // Disable legacy mode
  locale: config.settings.language, // Use language setting from config store
  fallbackLocale: "en",
  messages: {
    en,
    zh,
    es,
    fr,
    de,
    ja,
    ko,
    ru,
    pt
  },
})

// Set up global properties
app.config.globalProperties.$invoke = invoke

// Use the router and i18n
app.use(router)
app.use(i18n)

// Mount the app
app.mount('#app')
console.log('App mounted', app)

// Listen for events
if (isMainWindow) {
  listen('settings-appearance-changed', (event) => {
    config.setAppearance(event.payload)
  })
  listen('settings-lightTheme-changed', (event) => {
    config.setLightTheme(event.payload)
  })
  listen('settings-darkTheme-changed', (event) => {
    config.setDarkTheme(event.payload)
  })
  listen('settings-scale-changed', (event) => {
    config.setScale(event.payload)
  })
  listen('settings-externalApps-changed', (event) => {
    config.setExternalApps(event.payload)
  })
  listen('settings-language-changed', (event) => {
    config.setLanguage(event.payload)
  })
  listen('settings-showToolTip-changed', (event) => {
    config.setShowToolTip(event.payload)
  })
  listen('settings-showStatusBar-changed', (event) => {
    config.setShowStatusBar(event.payload)
  })
  listen('settings-autoCheckUpdates-changed', (event) => {
    config.setAutoCheckUpdates(event.payload)
  })
  listen('settings-debugMode-changed', (event) => {
    config.setDebugMode(event.payload)
  })
  listen('settings-settingsTabIndex-changed', (event) => {
    config.setSettingsTabIndex(event.payload)
  })
  listen('settings-folderSort-changed', (event) => {
    config.setFolderSort(event.payload)
  })
  listen('settings-calendarSort-changed', (event) => {
    config.setCalendarSort(event.payload)
  })
  listen('settings-categorySort-changed', (event) => {
    config.setCategorySort(event.payload)
  })
  listen('settings-showSubfolderFiles-changed', (event) => {
    config.setShowSubfolderFiles(event.payload)
  })
  listen('settings-thumbnailSize-changed', (event) => {
    config.setThumbnailSize(event.payload)
  })
  listen('settings-rawThumbnailSource-changed', (event) => {
    config.setRawThumbnailSource(event.payload)
  })
  listen('settings-mapProvider-changed', (event) => {
    config.setMapProvider(event.payload)
  })
  listen('settings-tiandituToken-changed', (event) => {
    config.setTiandituToken(event.payload)
  })
  listen('settings-gridStyle-changed', (event) => {
    config.setGridStyle(event.payload)
  })
  listen('settings-gridScaling-changed', (event) => {
    config.setGridScaling(event.payload)
  })
  listen('settings-gridThumbnailCorners-changed', (event) => {
    config.setGridThumbnailCorners(event.payload)
  })
  listen('settings-gridLabelPrimary-changed', (event) => {
    config.setGridLabelPrimary(event.payload)
  })
  listen('settings-gridLabelSecondary-changed', (event) => {
    config.setGridLabelSecondary(event.payload)
  })
  listen('settings-gridThumbnailBadge-changed', (event) => {
    config.setGridThumbnailBadge(event.payload)
  })
  listen('settings-dblClickAction-changed', (event) => {
    config.setDblClickAction(event.payload)
  })
  listen('settings-showFilmStrip-changed', (event) => {
    config.setShowFilmStrip(event.payload)
  })
  listen('settings-filmStripViewPreviewPosition-changed', (event) => {
    config.setFilmStripViewPreviewPosition(event.payload)
  })
  listen('settings-mouseWheelMode-changed', (event) => {
    config.setMouseWheelMode(event.payload)
  })
  listen('settings-slideShowInterval-changed', (event) => {
    config.setSlideShowInterval(event.payload)
  })
  listen('settings-autoPlayVideo-changed', (event) => {
    config.setAutoPlayVideo(event.payload)
  })
  listen('settings-loopVideo-changed', (event) => {
    config.settings.loopVideo = event.payload
  })
  listen('settings-groupRawJpegPairs-changed', (event) => {
    config.settings.groupRawJpegPairs = event.payload
  })
  listen('settings-smallFileFilter-changed', (event) => {
    config.setSmallFileFilter(event.payload)
  })
  listen('settings-navigatorViewMode-changed', (event) => {
    config.setNavigatorViewMode(event.payload)
  })
  listen('settings-navigatorViewSize-changed', (event) => {
    config.setNavigatorViewSize(event.payload)
  })
  listen('settings-viewBackground-changed', (event) => {
    config.setViewBackground(event.payload)
  })
  listen('settings-slideShowTransition-changed', (event) => {
    config.setSlideShowTransition(event.payload)
  })
  listen('settings-imageSearchThresholdIndex-changed', (event) => {
    config.setImageSearchThresholdIndex(event.payload)
  })
  listen('settings-imageSearchModel-changed', (event) => {
    config.setImageSearchModel(event.payload)
  })
  listen('settings-similarPhotoGroupingThresholdIndex-changed', (event) => {
    config.setSimilarPhotoGroupingThresholdIndex(event.payload)
  })
  listen('settings-faceClusterThresholdIndex-changed', (event) => {
    config.setFaceClusterThresholdIndex(event.payload)
  })
  listen('settings-faceEnabled-changed', (event) => {
    config.setFaceEnabled(event.payload)
  })
  listen('libraries-changed', () => {
    config.notifyLibrariesChanged()
  })
}
