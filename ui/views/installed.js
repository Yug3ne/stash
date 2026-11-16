// Installed: everything you installed — packages (official, Omarchy repo, AUR;
// dependencies hidden) and AppImages — with Open, Update and Remove on each.

import { h, appIcon, spinner, emptyState, formatBytes } from '../ui.js';
import { packageButtons } from './discover.js';
import { appButtons } from './appimages.js';

let filter = '';
let sourceFilter = 'all'; // 'all' | 'updates' | 'repo' | 'omarchy' | 'aur' | 'appimage'

const TABS = { all: 'All', updates: 'Updates', repo: 'Official', omarchy: 'Omarchy', aur: 'AUR', appimage: 'AppImages' };
const SOURCE_LABELS = { repo: null, omarchy: 'omarchy', aur: 'aur', appimage: 'appimage' };

function render(container) {
  const list = h('div', { class: 'list' });
  const count = h('span', { class: 'muted' });
  const checking = h('span', { class: 'muted' });
  let items = []; // { kind: 'package' | 'appimage', name, source, latest, data }
  let latestByKey = {}; // `${source}:${name}` → newer version

  const latest = (item) => latestByKey[`${item.source === 'appimage' ? 'appimage' : 'pkg'}:${item.name}`];

  const show = () => {
    const q = filter.toLowerCase();
    const visible = items.filter(
      (item) =>
        (sourceFilter === 'all' ||
          (sourceFilter === 'updates' ? latest(item) : item.source === sourceFilter)) &&
        (item.name.toLowerCase().includes(q) ||
          item.label.toLowerCase().includes(q) ||
          (item.description ?? '').toLowerCase().includes(q)),
    );
    // Things with updates first, so they're easy to find.
    visible.sort((a, b) => Boolean(latest(b)) - Boolean(latest(a)));
    count.textContent = `${visible.length} of ${items.length}`;
    // Rendering ~1000 rows is slow and nobody scrolls that far; filter instead.
    list.replaceChildren(
      ...(visible.length ? visible.slice(0, 300).map((item) => row(item, latest(item), load)) : [emptyState('Nothing matches')]),
    );
  };

  const load = async () => {
    list.replaceChildren(spinner());
    try {
      const [packages, appimages] = await Promise.all([
        window.api.packages.installed(),
        window.api.appimages.list().catch(() => []),
      ]);
      items = [
        ...appimages.map((app) => ({
          kind: 'appimage',
          name: app.name,
          label: app.displayName,
          source: 'appimage',
          description: `${app.name} · ${formatBytes(app.size)}${app.repo ? ` · updates from ${app.repo}` : ''}`,
          version: app.version,
          icon: app.icon,
          data: app,
        })),
        ...packages.map((pkg) => ({
          kind: 'package',
          name: pkg.name,
          label: pkg.name,
          source: pkg.source,
          description: [pkg.description, pkg.size].filter(Boolean).join(' · '),
          version: pkg.version,
          icon: pkg.icon,
          data: pkg,
        })),
      ].sort((a, b) => a.label.localeCompare(b.label));
      show();
    } catch (error) {
      list.replaceChildren(emptyState('Could not list apps', error.message));
    }
    checkUpdates();
  };

  // Checking takes a few seconds (it syncs a copy of the package databases and
  // asks GitHub), so show the list first and add Update buttons when it's done.
  const checkUpdates = async () => {
    checking.textContent = 'Checking for updates…';
    const [packages, appimages] = await Promise.all([
      window.api.packages.checkUpdates().catch(() => []),
      window.api.appimages.checkUpdates().catch(() => []),
    ]);
    latestByKey = Object.fromEntries([
      ...packages.map((u) => [`pkg:${u.name}`, u.latest]),
      ...appimages.map((u) => [`appimage:${u.name}`, u.latest]),
    ]);
    const n = packages.length + appimages.length;
    checking.textContent = n ? `${n} update${n === 1 ? '' : 's'} available` : 'Everything is up to date';
    document.dispatchEvent(new CustomEvent('badge', { detail: { id: 'updates', count: n } }));
    if (container.isConnected) show();
  };

  const tabs = Object.entries(TABS).map(([value, label]) =>
    h(
      'button',
      {
        class: `tab ${sourceFilter === value ? 'active' : ''}`,
        onclick: (e) => {
          sourceFilter = value;
          tabs.forEach((t) => t.classList.remove('active'));
          e.currentTarget.classList.add('active');
          show();
        },
      },
      label,
    ),
  );

  container.append(
    h(
      'header',
      { class: 'view-header' },
      h('div', {}, h('h1', {}, 'Installed'), h('p', { class: 'subtitle' }, 'Apps and packages you installed — update or remove each one')),
      checking,
    ),
    h(
      'div',
      { class: 'toolbar' },
      h('input', {
        type: 'search',
        class: 'search grow',
        placeholder: 'Filter…',
        value: filter,
        oninput: (e) => {
          filter = e.target.value;
          show();
        },
      }),
      h('div', { class: 'tabs' }, tabs),
      count,
    ),
    list,
  );
  load();
}

function row(item, latest, refresh) {
  const label = SOURCE_LABELS[item.source];
  return h(
    'div',
    { class: 'row' },
    appIcon(item.icon, item.label),
    h(
      'div',
      { class: 'row-text' },
      h(
        'div',
        { class: 'row-title' },
        item.label,
        h('span', { class: 'version' }, item.version),
        latest && h('span', { class: 'source update' }, `→ ${latest}`),
        label && h('span', { class: `source ${item.source}` }, label),
      ),
      h('div', { class: 'row-desc' }, item.description),
    ),
    item.kind === 'appimage'
      ? appButtons(item.data, latest, refresh)
      : packageButtons(
          { name: item.name, label: item.label, installed: true, hasApp: item.data.hasApp, source: item.source, latest },
          refresh,
        ),
  );
}

export default { id: 'installed', title: 'Installed', icon: 'installed', render };
