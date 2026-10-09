// Everything the pages can ask the Rust side to do. Each function calls a
// `#[tauri::command]` in src/gui.rs with the same name.

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

window.api = {
  theme: () => invoke('theme'),
  catalog: () => invoke('catalog'),
  showFolder: (path) => invoke('show_folder', { path }),
  openUrl: (url) => invoke('open_url', { url }),

  appimages: {
    list: () => invoke('appimages_list'),
    checkUpdates: () => invoke('appimages_check_updates'),
    install: (source, options) => invoke('appimages_install', { source, options }),
    update: (name) => invoke('appimages_update', { name }),
    updateAll: () => invoke('appimages_update_all'),
    remove: (name) => invoke('appimages_remove', { name }),
    launch: (name) => invoke('appimages_launch', { name }),
    pickFile: () => invoke('pick_appimage'),
  },

  packages: {
    search: (query) => invoke('packages_search', { query }),
    installed: () => invoke('packages_installed'),
    status: (names) => invoke('packages_status', { names }),
    checkUpdates: () => invoke('packages_check_updates'),
    systemInfo: () => invoke('system_info'),
    install: (names) => invoke('packages_install', { names }),
    remove: (names) => invoke('packages_remove', { names }),
    launch: (name) => invoke('packages_launch', { name }),
    upgrade: () => invoke('packages_upgrade'),
    removeOrphans: () => invoke('packages_remove_orphans'),
    cleanCache: () => invoke('packages_clean_cache'),
  },

  /**
   * Progress from running tasks: { kind: 'step'|'done'|'warn', value: text }
   * or { kind: 'progress', value: 0-100 }.
   */
  onTask: (callback) => listen('task', (event) => callback(event.payload)),

  /** Files dragged onto the window: callback({ type: 'over'|'drop'|'leave', paths }). */
  onFileDrop: (callback) =>
    window.__TAURI__.webviewWindow.getCurrentWebviewWindow().onDragDropEvent((event) => callback(event.payload)),
};

// Errors from Rust arrive as plain strings; make them Errors so `.message` works.
for (const group of [window.api, window.api.appimages, window.api.packages]) {
  for (const [key, fn] of Object.entries(group)) {
    if (typeof fn !== 'function' || key.startsWith('on')) continue;
    group[key] = (...args) =>
      fn(...args).catch((error) => {
        throw error instanceof Error ? error : new Error(String(error));
      });
  }
}
