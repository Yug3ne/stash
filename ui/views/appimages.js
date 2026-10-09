// AppImages: install from a file, a URL, or a GitHub repo, and manage the
// AppImages installed by appimage-install.

import { h, runTask, toast, appIcon, formatBytes, spinner, emptyState, confirmDialog } from '../ui.js';

let updatesByName = {}; // name → latest version, filled by "Check for updates"

// The window reports dropped files once, globally; this view handles them
// only while it's on screen.
let handleDrop = null;
window.api.onFileDrop((event) => {
  if (document.querySelector('.drop-zone')) handleDrop?.(event);
});

function render(container) {
  const list = h('div', { class: 'list' });

  container.append(
    h(
      'header',
      { class: 'view-header' },
      h('div', {}, h('h1', {}, 'AppImages'), h('p', { class: 'subtitle' }, 'Extracted once, launched fast — no FUSE, no RAM overhead')),
      h(
        'div',
        { class: 'button-row' },
        h('button', { class: 'ghost', onclick: () => checkUpdates(list) }, 'Check for updates'),
        h('button', { onclick: () => updateAll(list) }, 'Update all'),
      ),
    ),
    installPanel(() => loadList(list)),
    h('h2', {}, 'Installed'),
    list,
  );
  loadList(list);
}

// ─── Install panel ───────────────────────────────────────────────────────────

function installPanel(onInstalled) {
  const options = {
    name: h('input', { type: 'text', placeholder: 'auto', spellcheck: false }),
    repo: h('input', { type: 'text', placeholder: 'owner/repo (auto for most apps)', spellcheck: false }),
    optimize: h('input', { type: 'checkbox', checked: true }),
    noSandbox: h('input', { type: 'checkbox' }),
    keep: h('input', { type: 'checkbox' }),
  };
  const source = h('input', {
    type: 'text',
    class: 'grow',
    placeholder: 'GitHub repo (owner/repo) or a link to an .AppImage',
    spellcheck: false,
    onkeydown: (e) => e.key === 'Enter' && installFrom(source.value.trim()),
  });

  const installFrom = async (from) => {
    if (!from) return;
    const label = from.split('/').pop();
    const ok = await runTask(
      `Installing ${label}`,
      () =>
        window.api.appimages.install(from, {
          name: options.name.value.trim() || undefined,
          repo: options.repo.value.trim() || undefined,
          optimize: options.optimize.checked,
          noSandbox: options.noSandbox.checked,
          keep: options.keep.checked,
        }),
      { success: `Installed ${label}`, showLog: true },
    );
    if (ok) {
      source.value = '';
      options.name.value = '';
      options.repo.value = '';
      onInstalled();
    }
  };

  const pickFile = async () => {
    const file = await window.api.appimages.pickFile();
    if (file) installFrom(file);
  };

  // Drag & drop an .AppImage anywhere on the window (see onFileDrop below).
  const dropZone = h(
    'div',
    { class: 'drop-zone', onclick: pickFile },
    h('div', { class: 'drop-title' }, 'Drop an AppImage here'),
    h('div', { class: 'drop-hint' }, 'or click to choose a file'),
  );
  handleDrop = ({ type, paths }) => {
    dropZone.classList.toggle('hover', type === 'over' || type === 'enter');
    if (type !== 'drop' || !paths?.length) return;
    const file = paths[0];
    if (!/\.appimage$/i.test(file)) {
      toast(`${file.split('/').pop()} is not an AppImage`, 'error');
      return;
    }
    installFrom(file);
  };

  const option = (input, label, hint) =>
    h('label', { class: 'option', title: hint }, input, h('span', {}, label));

  return h(
    'section',
    { class: 'panel install-panel' },
    dropZone,
    h(
      'div',
      { class: 'install-side' },
      h('div', { class: 'input-row' }, source, h('button', { class: 'primary', onclick: () => installFrom(source.value.trim()) }, 'Install')),
      h(
        'div',
        { class: 'options' },
        option(options.optimize, 'Optimize', 'Strip debug symbols and unused languages — smaller and faster to load'),
        option(options.noSandbox, 'No sandbox', 'Some Electron apps only start with --no-sandbox'),
        option(options.keep, 'Keep .AppImage file', 'Otherwise the original file is deleted after installing'),
      ),
      h(
        'div',
        { class: 'field-row' },
        h('label', { class: 'field' }, h('span', {}, 'Command name'), options.name),
        h('label', { class: 'field' }, h('span', {}, 'Update from GitHub'), options.repo),
      ),
    ),
  );
}

// ─── Installed list ──────────────────────────────────────────────────────────

async function loadList(list) {
  list.replaceChildren(spinner());
  let apps;
  try {
    apps = await window.api.appimages.list();
  } catch (error) {
    list.replaceChildren(emptyState('Could not list AppImages', error.message));
    return;
  }
  if (apps.length === 0) {
    list.replaceChildren(emptyState('No AppImages installed yet', 'Drop one above, or paste a GitHub repo like “marktext/marktext”.'));
    return;
  }
  list.replaceChildren(...apps.map((app) => appRow(app, () => loadList(list))));
}

function appRow(app, refresh) {
  const latest = updatesByName[app.name];
  return h(
    'div',
    { class: 'row' },
    appIcon(app.icon, app.displayName),
    h(
      'div',
      { class: 'row-text' },
      h(
        'div',
        { class: 'row-title' },
        app.displayName,
        h('span', { class: 'version' }, app.version),
        latest && h('span', { class: 'source update' }, `→ ${latest}`),
        app.optimized && h('span', { class: 'source' }, 'optimized'),
      ),
      h(
        'div',
        { class: 'row-desc' },
        `${app.name} · ${formatBytes(app.size)}`,
        app.repo ? ` · updates from ${app.repo}` : ' · no update source',
      ),
    ),
    h(
      'div',
      { class: 'button-row' },
      h('button', { class: 'small', onclick: () => window.api.appimages.launch(app.name) }, 'Open'),
      app.repo &&
        h(
          'button',
          {
            class: latest ? 'primary small' : 'ghost small',
            onclick: async () => {
              if (await runTask(`Updating ${app.displayName}`, () => window.api.appimages.update(app.name), { success: `${app.displayName} is up to date`, showLog: true })) {
                delete updatesByName[app.name];
                document.dispatchEvent(new Event('updates:changed'));
                refresh();
              }
            },
          },
          'Update',
        ),
      h('button', { class: 'ghost small', title: 'Show files', onclick: () => window.api.showFolder(app.path) }, 'Files'),
      h(
        'button',
        {
          class: 'ghost small danger-text',
          onclick: async () => {
            const ok = await confirmDialog({
              title: `Remove ${app.displayName}?`,
              message: 'The app, its menu entry and its command will be deleted. Your settings in ~/.config are kept.',
              confirm: 'Remove',
              danger: true,
            });
            if (ok && (await runTask(`Removing ${app.displayName}`, () => window.api.appimages.remove(app.name), { success: `${app.displayName} removed` }))) {
              refresh();
            }
          },
        },
        'Remove',
      ),
    ),
  );
}

async function checkUpdates(list) {
  await runTask('Checking AppImages for updates', async () => {
    const updates = await window.api.appimages.checkUpdates();
    updatesByName = Object.fromEntries(updates.map((u) => [u.name, u.latest]));
    toast(updates.length ? `${updates.length} update(s) available` : 'All AppImages are up to date', 'success');
  });
  loadList(list);
}

async function updateAll(list) {
  if (await runTask('Updating all AppImages', () => window.api.appimages.updateAll(), { success: 'AppImages updated', showLog: true })) {
    updatesByName = {};
    document.dispatchEvent(new Event('updates:changed'));
  }
  loadList(list);
}

export default { id: 'appimages', title: 'AppImages', icon: 'appimages', render };
