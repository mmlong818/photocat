<template>
  <div
    class="photo-map-view group/map relative flex-1 overflow-hidden"
    @mouseenter="uiStore.setMapActive(true)"
    @mouseleave="uiStore.setMapActive(false)"
  >
    <div v-if="loading" class="absolute inset-0 z-50 flex items-center justify-center bg-base-200/50">
      <span class="loading loading-spinner loading-md text-primary"></span>
    </div>
    <div ref="mapEl" class="h-full w-full"></div>
    <div class="absolute top-2 left-2 z-500 flex cursor-pointer rounded-box bg-base-100/30 opacity-0 pointer-events-none transition-opacity duration-150 group-hover/map:bg-base-100/70 group-hover/map:opacity-100 group-hover/map:pointer-events-auto">
      <TButton :icon="IconZoomOut" :tooltip="t('map.zoom_out')" :disabled="zoom <= 0" @click="zoomOut" />
      <TButton :icon="IconZoomIn" :tooltip="t('map.zoom_in')" :disabled="zoom >= activeMaxZoom" @click="zoomIn" />
      <TButton :icon="IconMapCenter" :tooltip="t('map.zoom_center')" @click="isQueryMap ? fitBounds() : zoomCenter()" />
      <TButton
        :icon="config.infoPanel.mapTheme === 0 ? IconMapDefault : IconMapSatellite"
        :tooltip="t(config.infoPanel.mapTheme === 0 ? 'map.standard' : 'map.satellite')"
        @click="toggleMap"
      />
      <TButton v-if="showAppleMapsButton" :icon="IconExternal" :tooltip="t('file_info.open_apple_maps')" @click="openAppleMaps" />
    </div>
    <div v-if="isQueryMap && !loading" class="absolute top-2 right-2 z-500 pointer-events-none rounded-box bg-base-100/60 px-2 py-1 text-xs text-base-content/70">
      {{ points.length > 0 ? t('map.photo_count', { count: totalCount.toLocaleString() }) : t('map.no_photos_in_view') }}
    </div>
  </div>
</template>

<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import L from 'leaflet'
import 'leaflet/dist/leaflet.css'

import { config } from '@/common/config'
import { createTileLayerGroup, getGlobalMapTheme, getMapTheme } from '@/common/mapProviders'
import {
  getCollectionQueryFileIds,
  getFilesByIds,
  getGpsMapPoints,
  getQueryFiles,
  getSmartQueryFileIds,
  openExternalUrl,
} from '@/common/api'
import { getThumbUrl, isMac } from '@/common/utils'
import { useUIStore } from '@/stores/uiStore'
import { IconExternal, IconMapCenter, IconMapDefault, IconMapSatellite, IconZoomIn, IconZoomOut } from '@/common/icons'
import TButton from '@/components/TButton.vue'

const props = defineProps({
  queryParams: { type: Object, default: null },
  querySource: { type: String, default: 'query' },
  collectionId: { type: Number, default: null },
  fileIds: { type: Array, default: () => [] },
  restoreView: { type: Object, default: null },
  active: { type: Boolean, default: true },
  lat: { type: Number, default: 0 },
  lon: { type: Number, default: 0 },
  label: { type: String, default: '猫叔的图' },
})
const emit = defineEmits(['open-cluster', 'select-file', 'preview-file', 'restored'])

const { t } = useI18n()
const uiStore = useUIStore()
const mapEl = ref(null)
const loading = ref(true)
const points = ref([])
const zoom = ref(2)
const singleMarker = ref(null)
const totalCount = computed(() => points.value.reduce((sum, point) => sum + point.count, 0))
const isQueryMap = computed(() => props.queryParams !== null)
const showAppleMapsButton = computed(() => !isQueryMap.value && isMac && validLatLon(props.lat, props.lon))

const DETAIL_ZOOM = 13
const DETAIL_LIMIT = 500
const CLUSTER_SIZE = 76
// Leaflet resolves a one-point bounds to its maximum zoom. Keep surrounding
// map context and avoid requesting an unsupported raster detail level instead.
const SINGLE_POINT_FIT_ZOOM = 13
const activeMaxZoom = ref(19)

let map = null
let markerLayer = null
let tileLayer = null
let resizeObserver = null
let tileErrorFallbackTriggered = false
let pointRequestToken = 0
let detailRequestToken = 0
let detailTimer = null
let visibleFiles = []
let sourceFiles = null
let needsRefresh = false

onMounted(async () => {
  map = L.map(mapEl.value, { center: [20, 0], zoom: 2, keyboard: false, zoomControl: false, maxZoom: activeMaxZoom.value })
  map.attributionControl.setPrefix('')
  L.control.scale({ position: 'bottomleft', imperial: false }).addTo(map)
  markerLayer = L.layerGroup().addTo(map)
  map.on('zoomend', onMapChanged)
  map.on('moveend', onMapChanged)
  resizeObserver = new ResizeObserver(() => map?.invalidateSize())
  resizeObserver.observe(mapEl.value)
  updateTheme()
  window.addEventListener('keydown', handleMapKeyDown, true)
  if (props.active && isQueryMap.value) {
    await loadPoints(true)
  }
  else if (props.active) updateFromCoords()
  else loading.value = false
  requestAnimationFrame(() => map?.invalidateSize())
})

onBeforeUnmount(() => {
  uiStore.setMapActive(false)
  window.removeEventListener('keydown', handleMapKeyDown, true)
  if (detailTimer) clearTimeout(detailTimer)
  resizeObserver?.disconnect()
  map?.remove()
})

watch(() => [config.infoPanel.mapTheme, config.settings.mapProvider, config.settings.tiandituToken], updateTheme)
watch(() => [props.queryParams, props.querySource, props.collectionId, props.fileIds], () => {
  if (!props.active) {
    needsRefresh = true
    return
  }
  needsRefresh = false
  if (isQueryMap.value) void loadPoints(true)
  else updateFromCoords()
}, { deep: true })
watch(() => [props.lat, props.lon], () => {
  if (props.active && !isQueryMap.value) updateFromCoords()
})
watch(() => props.active, (active) => {
  if (!active) {
    pointRequestToken++
    detailRequestToken++
    if (detailTimer) clearTimeout(detailTimer)
    return
  }
  void nextTick(async () => {
    if (!map || !props.active) return
    map.invalidateSize()
    if (props.restoreView || needsRefresh) {
      needsRefresh = false
      await loadPoints(true)
      return
    }
    zoom.value = map.getZoom()
    renderMarkers()
    scheduleDetailFetch()
  })
})

async function loadPoints(fitToResults) {
  if (!map) return
  const token = ++pointRequestToken
  const restoreView = fitToResults ? props.restoreView : null
  loading.value = true
  try {
    const result = await getMapPoints()
    if (token !== pointRequestToken || !props.active) return
    points.value = result || []
    visibleFiles = []
    if (fitToResults) {
      if (restoreView) {
        map.setView([restoreView.lat, restoreView.lon], restoreView.zoom, { animate: false })
        emit('restored')
      } else if (points.value.length > 0) {
        fitMapToPoints()
      } else {
        map.setView([20, 0], 2, { animate: false })
      }
    }
    requestAnimationFrame(() => map?.invalidateSize())
    zoom.value = map.getZoom()
    renderMarkers()
    scheduleDetailFetch()
  } finally {
    if (token === pointRequestToken) loading.value = false
  }
}

function updateFromCoords() {
  if (!map) return
  if (singleMarker.value) {
    markerLayer.removeLayer(singleMarker.value)
    singleMarker.value = null
  }
  if (validLatLon(props.lat, props.lon)) {
    singleMarker.value = L.marker([props.lat, props.lon]).addTo(markerLayer)
    map.setView([props.lat, props.lon], zoom.value)
  } else {
    map.setView([0, 0], 2)
  }
  loading.value = false
}

function onMapChanged() {
  zoom.value = map.getZoom()
  if (!isQueryMap.value) return
  renderMarkers()
  scheduleDetailFetch()
}

function scheduleDetailFetch() {
  if (zoom.value < DETAIL_ZOOM) return
  if (detailTimer) clearTimeout(detailTimer)
  detailTimer = setTimeout(fetchVisibleFiles, 200)
}

async function fetchVisibleFiles() {
  if (!map || zoom.value < DETAIL_ZOOM) return
  if (getVisiblePointCount() > DETAIL_LIMIT) {
    visibleFiles = []
    renderMarkers()
    return
  }
  if (sourceFiles) {
    const bounds = map.getBounds()
    visibleFiles = sourceFiles.filter(file => (
      file.gps_latitude != null
      && file.gps_longitude != null
      && bounds.contains([file.gps_latitude, file.gps_longitude])
    ))
    renderMarkers()
    return
  }
  const token = ++detailRequestToken
  const bounds = map.getBounds()
  const files = await getQueryFiles({
    ...props.queryParams,
    gpsMinLat: bounds.getSouth(),
    gpsMaxLat: bounds.getNorth(),
    gpsMinLon: bounds.getWest(),
    gpsMaxLon: bounds.getEast(),
  }, 0, DETAIL_LIMIT)
  if (token !== detailRequestToken) return
  visibleFiles = files || []
  renderMarkers()
}

async function getMapPoints() {
  sourceFiles = null
  if (props.querySource === 'collection' && props.collectionId) {
    const ids = await getCollectionQueryFileIds(props.collectionId, props.queryParams)
    sourceFiles = await getFilesByIds(ids || [])
  } else if (props.querySource === 'smart') {
    const ids = await getSmartQueryFileIds(props.queryParams)
    sourceFiles = await getFilesByIds(ids || [])
  } else if (props.querySource === 'search') {
    sourceFiles = await getFilesByIds(props.fileIds)
  } else {
    return getGpsMapPoints(props.queryParams)
  }
  return aggregateFiles(sourceFiles || [])
}

function aggregateFiles(files) {
  const cells = new Map()
  for (const file of files) {
    // Number(null) is 0, which would incorrectly place photos without GPS data
    // at the equator/prime meridian and include them in a map cluster.
    if (file.gps_latitude == null || file.gps_longitude == null || file.gps_latitude === '' || file.gps_longitude === '') continue
    const lat = Number(file.gps_latitude)
    const lon = Number(file.gps_longitude)
    if (!Number.isFinite(lat) || !Number.isFinite(lon)) continue
    const key = `${Math.round(lat * 100)}:${Math.round(lon * 100)}`
    const cell = cells.get(key) || { lat: 0, lon: 0, count: 0, file_id: Number(file.id) }
    cell.lat += lat
    cell.lon += lon
    cell.count++
    cell.file_id = Math.min(cell.file_id, Number(file.id))
    cells.set(key, cell)
  }
  return [...cells.values()].map(cell => ({ ...cell, lat: cell.lat / cell.count, lon: cell.lon / cell.count }))
}

function renderMarkers() {
  if (!map || !markerLayer) return
  markerLayer.clearLayers()
  if (zoom.value >= DETAIL_ZOOM && visibleFiles.length > 0 && getVisiblePointCount() <= DETAIL_LIMIT) {
    for (const file of visibleFiles) addPhotoMarker(file.gps_latitude, file.gps_longitude, file.id, 1)
    return
  }

  const clusters = new Map()
  const bounds = map.getBounds()
  for (const point of points.value) {
    if (!bounds.contains([point.lat, point.lon])) continue
    const pixel = map.project([point.lat, point.lon], map.getZoom())
    const key = `${Math.floor(pixel.x / CLUSTER_SIZE)}:${Math.floor(pixel.y / CLUSTER_SIZE)}`
    const cluster = clusters.get(key)
    if (!cluster) {
      clusters.set(key, {
        ...point,
        representativeCount: point.count,
        minLat: point.lat - 0.01,
        maxLat: point.lat + 0.01,
        minLon: point.lon - 0.01,
        maxLon: point.lon + 0.01,
      })
    } else {
      cluster.count += point.count
      cluster.minLat = Math.min(cluster.minLat, point.lat - 0.01)
      cluster.maxLat = Math.max(cluster.maxLat, point.lat + 0.01)
      cluster.minLon = Math.min(cluster.minLon, point.lon - 0.01)
      cluster.maxLon = Math.max(cluster.maxLon, point.lon + 0.01)
      if (point.count > cluster.representativeCount) {
        cluster.file_id = point.file_id
        cluster.lat = point.lat
        cluster.lon = point.lon
        cluster.representativeCount = point.count
      }
    }
  }
  for (const cluster of clusters.values()) addPhotoMarker(cluster.lat, cluster.lon, cluster.file_id, cluster.count, cluster)
}

function getVisiblePointCount() {
  if (!map) return 0
  const bounds = map.getBounds()
  return points.value.reduce((count, point) => (
    bounds.contains([point.lat, point.lon]) ? count + point.count : count
  ), 0)
}

function addPhotoMarker(lat, lon, fileId, count, cluster = null) {
  if (lat == null || lon == null) return
  const icon = L.divIcon({
    className: 'map-photo-marker-wrapper',
    iconSize: [64, 72],
    iconAnchor: [32, 72],
    html: `<div class="map-photo-marker"><img src="${getThumbUrl(fileId, false, config.settings.thumbnailSize || 512)}" />${count > 1 ? `<span>${count > 999 ? '999+' : count}</span>` : ''}</div>`,
  })
  const marker = L.marker([lat, lon], { icon, keyboard: false }).addTo(markerLayer)
  marker.on('click', () => {
    if (!cluster || Number(count) === 1) {
      emit('select-file', fileId)
      return
    }
    emit('open-cluster', {
      ...(cluster || {
      minLat: lat - 0.0001,
      maxLat: lat + 0.0001,
      minLon: lon - 0.0001,
      maxLon: lon + 0.0001,
      count,
      }),
      view: { lat: map.getCenter().lat, lon: map.getCenter().lng, zoom: map.getZoom() },
    })
  })
  marker.on('dblclick', (event) => {
    if (cluster && Number(count) !== 1) return
    L.DomEvent.stop(event.originalEvent)
    emit('preview-file', fileId)
  })
}

function updateTheme() {
  const theme = getMapTheme(config.settings.mapProvider, config.settings.tiandituToken, config.infoPanel.mapTheme)
  applyTheme(theme, false)
}

function applyTheme(theme, isFallback) {
  if (!map) return
  if (tileLayer) map.removeLayer(tileLayer)
  activeMaxZoom.value = theme.maxZoom
  map.setMaxZoom(theme.maxZoom)
  if (map.getZoom() > theme.maxZoom) map.setZoom(theme.maxZoom)
  if (!isFallback) tileErrorFallbackTriggered = false

  const created = createTileLayerGroup(L, theme)
  const activeLayer = created.layer
  tileLayer = activeLayer.addTo(map)
  created.tileLayers.forEach(layer => {
    layer.on('tileerror', () => {
      // Requests from a removed layer can still fail after a provider switch.
      // Ignore them so an old OSM request cannot replace the new provider.
      if (tileLayer !== activeLayer || tileErrorFallbackTriggered || isFallback) return
      tileErrorFallbackTriggered = true
      applyTheme(getGlobalMapTheme(config.infoPanel.mapTheme), true)
    })
  })
}

function zoomIn() { if (map && zoom.value < activeMaxZoom.value) map.setZoom(zoom.value + 1) }
function zoomOut() { if (map && zoom.value > 0) map.setZoom(zoom.value - 1) }
function fitBounds() {
  if (!map || points.value.length === 0) return map?.setView([20, 0], 2)
  fitMapToPoints()
}
function fitMapToPoints() {
  map.fitBounds(L.latLngBounds(points.value.map(point => [point.lat, point.lon])), {
    padding: [20, 20],
    maxZoom: points.value.length === 1 ? Math.min(SINGLE_POINT_FIT_ZOOM, activeMaxZoom.value) : activeMaxZoom.value,
  })
}
function zoomCenter() {
  zoom.value = 13
  updateFromCoords()
}
function toggleMap() { config.infoPanel.mapTheme = config.infoPanel.mapTheme === 0 ? 1 : 0 }
function validLatLon(lat, lon) { return lat != null && lon != null && lat >= -90 && lat <= 90 && lon >= -180 && lon <= 180 }
async function openAppleMaps() {
  if (!showAppleMapsButton.value) return
  const label = props.label.trim() || '猫叔的图'
  await openExternalUrl(`maps://?ll=${props.lat},${props.lon}&q=${encodeURIComponent(label)}`)
}
function handleMapKeyDown(event) {
  const target = event.target
  if (target?.tagName === 'INPUT' || target?.tagName === 'TEXTAREA' || target?.isContentEditable) return
  if (!uiStore.mapActive || event.metaKey || event.ctrlKey || event.altKey) return
  if (event.key === '=') { event.preventDefault(); zoomIn() }
  if (event.key === '-') { event.preventDefault(); zoomOut() }
}
</script>
