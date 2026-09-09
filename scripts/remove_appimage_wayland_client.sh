#!/usr/bin/env bash
# Post-process AppImages after Tauri/linuxdeploy builds them.
#
# 1. Remove the bundled Wayland libraries so the host's ABI-compatible copies
#    are resolved instead (required for EGL/Mesa on pure-Wayland systems).
# 2. Disable AppImageKit's GStreamer plugin-path override, since the AppImage
#    does not bundle GStreamer plugins (fixes "GStreamer element appsink not
#    found").
# 3. Remove bundled host-runtime libraries that linuxdeploy follows through
#    WebKitGTK (GStreamer, GLib, and their transitive dependencies). This
#    keeps GTK, WebKitGTK, GStreamer, and GLib on a consistent host ABI.
# 4. Optionally embed update information and generate a .zsync delta file.
#
# Usage:
#   remove_appimage_wayland_client.sh <appimage-directory>
#
# Optional environment variables (may contain `{name}` / `{name_star}`, replaced
# with each AppImage's basename):
#   UPDATE_INFO  appimagetool `-u` value (e.g. "zsync|https://.../{name}.zsync")
#   ZSYNC_URL    full-download URL written into the generated .zsync file
#   VERSION      version string; enables `{name_star}` (basename with the
#                version replaced by `*`, for gh-releases-zsync patterns)
#
# Note: repacking swaps the AppImage's embedded runtime for appimagetool's own.
# This is harmless — both are backward-compatible type-2 runtimes.
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "Usage: $0 <appimage-directory>" >&2
  exit 64
fi

APPIMAGE_DIR="$1"
UPDATE_INFO="${UPDATE_INFO:-}"
ZSYNC_URL="${ZSYNC_URL:-}"
VERSION="${VERSION:-}"

if [ ! -d "$APPIMAGE_DIR" ]; then
  echo "AppImage directory does not exist: $APPIMAGE_DIR" >&2
  exit 66
fi
# Resolve to an absolute path: the extraction subshell `cd`s away, so a relative
# path (as passed by the CI workflow) would no longer resolve.
APPIMAGE_DIR="$(cd "$APPIMAGE_DIR" && pwd)"

case "$(uname -m)" in
  x86_64|aarch64) APPIMAGE_ARCH="$(uname -m)" ;;
  *)
    echo "Unsupported AppImage architecture: $(uname -m)" >&2
    exit 69
    ;;
esac

WORK_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

APPIMAGETOOL="$WORK_DIR/appimagetool.AppImage"
curl --fail --location --silent --show-error \
  --output "$APPIMAGETOOL" \
  "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${APPIMAGE_ARCH}.AppImage"
chmod +x "$APPIMAGETOOL"
export APPIMAGE_EXTRACT_AND_RUN=1

shopt -s nullglob
APPIMAGES=("$APPIMAGE_DIR"/*.AppImage)
if [ "${#APPIMAGES[@]}" -eq 0 ]; then
  echo "No AppImages found in: $APPIMAGE_DIR" >&2
  exit 66
fi

for appimage in "${APPIMAGES[@]}"; do
  image_name="$(basename "$appimage")"
  image_work_dir="$WORK_DIR/${image_name}.work"
  mkdir "$image_work_dir"

  echo "==> Extracting ${image_name}"
  (
    cd "$image_work_dir"
    "$appimage" --appimage-extract >/dev/null
  )

  # Locate the bundled Wayland client library (the exact path varies by build,
  # so search rather than hardcoding usr/lib/).
  wayland_client="$(find "$image_work_dir/squashfs-root" -name 'libwayland-client.so.0' -print -quit)"
  if [ -z "$wayland_client" ]; then
    echo "Bundled libwayland-client.so.0 not found in ${image_name}" >&2
    exit 1
  fi
  echo "==> Removing bundled libwayland-client.so.0 from ${image_name}"
  rm -f "$wayland_client"

  # AppImageKit's AppRun.wrapped unconditionally sets
  # GST_PLUGIN_SYSTEM_PATH_1_0, even when Tauri's bundleMediaFramework is false
  # (tauri#15665). The bundled path does not exist, which prevents GStreamer
  # from finding host plugins such as appsink.
  #
  # AppRun.wrapped is an ELF binary, so line-oriented tools such as sed corrupt
  # it. Replace the environment-variable name with a same-length, app-specific
  # unused name. This is binary-safe and leaves the executable layout intact.
  apprun_wrapped="$image_work_dir/squashfs-root/AppRun.wrapped"
  if [ ! -f "$apprun_wrapped" ]; then
    echo "AppRun.wrapped not found in ${image_name}" >&2
    exit 1
  fi
  gst_path_var='GST_PLUGIN_SYSTEM_PATH_1_0'
  disabled_gst_path_var='LAP_IGNORE_GST_PLUGIN_PATH'
  if [ "${#gst_path_var}" -ne "${#disabled_gst_path_var}" ]; then
    echo "Internal error: replacement GStreamer variable has a different length" >&2
    exit 1
  fi
  if LC_ALL=C grep -aq "$gst_path_var" "$apprun_wrapped"; then
    echo "==> Disabling AppImageKit GStreamer plugin-path override in ${image_name}"
    LC_ALL=C perl -0777 -i -pe "s/${gst_path_var}/${disabled_gst_path_var}/g" "$apprun_wrapped"
    if LC_ALL=C grep -aq "$gst_path_var" "$apprun_wrapped" || \
      ! LC_ALL=C grep -aq "$disabled_gst_path_var" "$apprun_wrapped"; then
      echo "Failed to disable GStreamer plugin path override in ${image_name}" >&2
      exit 1
    fi
  else
    # Upstream (Tauri/linuxdeploy) may stop hardcoding this variable in a future
    # version; treat its absence as a no-op rather than failing the build.
    echo "GStreamer plugin path override not found in ${image_name}; nothing to do" >&2
  fi

  # linuxdeploy follows WebKitGTK's ELF dependencies and copies GStreamer, GLib,
  # and their transitive support libraries. It does not discover GStreamer's
  # runtime-loaded plugins or scanner, so a bundled core and host plugins can
  # have incompatible ABIs. A bundled older library can likewise be
  # incompatible with whatever newer host library ends up loading it: the host
  # libgio needs a libmount exporting MOUNT_2_40, the host libglib needs a
  # versioned libpcre2-8, and so on. Remove these partial stacks so GTK,
  # WebKitGTK, GStreamer, and GLib all resolve their common runtime libraries
  # from the host (full set validated in tauri-apps/tauri#15665).
  host_runtime_lib_patterns=(
    'libwayland-cursor.so*'
    'libwayland-egl.so*'
    'libwayland-server.so*'
    'libgst*.so*'
    'libgstreamer-*.so*'
    'liborc-*.so*'
    'libglib-2.0.so*'
    'libgobject-2.0.so*'
    'libgio-2.0.so*'
    'libgmodule-2.0.so*'
    'libgthread-2.0.so*'
    'libffi.so*'
    'libmount.so*'
    'libblkid.so*'
    'libselinux.so*'
    'libpcre2-8.so*'
    'libzstd.so*'
    'libelf.so*'
  )
  host_runtime_libs=()
  for host_runtime_lib_pattern in "${host_runtime_lib_patterns[@]}"; do
    while IFS= read -r -d '' host_runtime_lib; do
      host_runtime_libs+=("$host_runtime_lib")
    done < <(find "$image_work_dir/squashfs-root/usr/lib" -maxdepth 1 -type f -name "$host_runtime_lib_pattern" -print0)
  done
  if [ "${#host_runtime_libs[@]}" -gt 0 ]; then
    echo "==> Removing partial bundled host runtime libraries from ${image_name}"
    rm -f "${host_runtime_libs[@]}"
  else
    echo "No bundled host runtime libraries found in ${image_name}; nothing to do" >&2
  fi

  repack_args=(--no-appstream)
  if [ -n "$UPDATE_INFO" ]; then
    update_info="${UPDATE_INFO//\{name\}/$image_name}"
    if [ -n "$VERSION" ]; then
      name_star="${image_name/${VERSION}/*}"
      update_info="${update_info//\{name_star\}/$name_star}"
    fi
    repack_args+=(-u "$update_info")
  fi

  echo "==> Repacking ${image_name}"
  replacement="$image_work_dir/${image_name}.new"
  "$APPIMAGETOOL" "${repack_args[@]}" \
    "$image_work_dir/squashfs-root" "$replacement" >/dev/null
  chmod +x "$replacement"
  mv "$replacement" "$appimage"

  if [ -n "$ZSYNC_URL" ]; then
    zsync_url="${ZSYNC_URL//\{name\}/$image_name}"
    echo "==> Generating .zsync for ${image_name}"
    zsyncmake -u "$zsync_url" -o "$appimage.zsync" "$appimage"
  fi
done
