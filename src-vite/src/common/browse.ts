// Browsing a folder straight off disk, with nothing added to the library.
//
// Everything else in the app addresses a picture by its database row id. These
// helpers address pictures by path instead, so a folder can be looked at
// without importing it first. See src-tauri/src/t_browse.rs for the other half.

import { invoke } from '@tauri-apps/api/core';
import { convertFileSrc } from '@tauri-apps/api/core';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { isWin, isMac, shouldUseBackendPreview } from '@/common/utils';

export interface BrowseFile {
  name: string;
  path: string;
  size: number;
  /** Seconds since the epoch. */
  modified: number;
  /** 1 = image, 2 = video, 3 = RAW, matching the rest of the app. */
  file_type: number;
  /** Changes whenever the file does, so cached thumbnails stay honest. */
  signature: string;
}

export interface BrowseFolder {
  name: string;
  path: string;
}

export interface BrowseListing {
  folder: string;
  parent: string | null;
  folders: BrowseFolder[];
  files: BrowseFile[];
}

export const BROWSE_WINDOW_LABEL = 'browse';

export const SORT_NAME = 0;
export const SORT_DATE = 1;
export const SORT_SIZE = 2;
export const SORT_TYPE = 3;

/**
 * List one folder. Throws with the backend's own message on failure, because
 * a browse window that cannot read a folder has nothing else to show and the
 * reason (gone, renamed, no permission) is the only useful thing left.
 */
export async function browseFolder(
  path: string,
  sort = SORT_NAME,
  descending = false,
): Promise<BrowseListing> {
  return await invoke('browse_folder', { path, sort, descending });
}

export async function browseParentOf(path: string): Promise<string | null> {
  try {
    return await invoke('browse_parent_of', { path });
  } catch (error) {
    console.error('browseParentOf error:', error);
    return null;
  }
}

function browseUrl(kind: 'thumb' | 'full', file: BrowseFile): string {
  const scheme = isWin ? 'http://browse.localhost' : 'browse://localhost';
  const params = new URLSearchParams({ path: file.path, v: file.signature });
  return `${scheme}/${kind}?${params.toString()}`;
}

/** Grid thumbnail. Generated and cached on first request by the backend. */
export function browseThumbUrl(file: BrowseFile): string {
  return browseUrl('thumb', file);
}

/**
 * Full-size image for the large view.
 *
 * Anything the webview can decode by itself is handed over as a plain file
 * URL, which skips a decode and a copy through the protocol handler. RAW,
 * HEIC, TIFF and the rest have to go through the backend.
 */
export function browseFullUrl(file: BrowseFile): string {
  if (shouldUseBackendPreview(file.path, file.file_type)) {
    return browseUrl('full', file);
  }
  return convertFileSrc(file.path);
}

/**
 * Open the browse window on a folder, optionally landing on one file.
 *
 * Reuses the window if it is already open, the same way the image viewer does,
 * so double-clicking a second photo does not pile up windows.
 */
export async function openBrowseWindow(folder: string, selectPath = ''): Promise<void> {
  const query = new URLSearchParams({ folder });
  if (selectPath) query.set('select', selectPath);

  const existing = await WebviewWindow.getByLabel(BROWSE_WINDOW_LABEL);
  if (existing) {
    await existing.emit('browse-goto', { folder, select: selectPath });
    await existing.show();
    await existing.unminimize().catch(() => {});
    await existing.setFocus();
    return;
  }

  const created = new WebviewWindow(BROWSE_WINDOW_LABEL, {
    url: `/browse?${query.toString()}`,
    title: 'Browse',
    width: 1100,
    height: 760,
    minWidth: 720,
    minHeight: 480,
    resizable: true,
    visible: false,
    transparent: true,
    decorations: isMac,
    ...(isMac && {
      titleBarStyle: 'overlay',
      hiddenTitle: true,
    }),
  });

  created.once('tauri://created', () => {
    void created.show();
    void created.setFocus();
  });
}
