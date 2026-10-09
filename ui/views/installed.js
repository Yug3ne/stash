// Installed: every package you installed on purpose (not dependencies),
// with a filter box and open/remove buttons.

import { h, appIcon, spinner, emptyState } from '../ui.js';
import { packageButtons } from './discover.js';

let filter = '';
let sourceFilter = 'all'; // 'all' | 'repo' | 'aur'

function render(container) {
  const list = h('div', { class: 'list' });
  const count = h('span', { class: 'muted' });
  let packages = [];

  const show = () => {
    const q = filter.toLowerCase();
    const visible = packages.filter(
      (p) =>
        (sourceFilter === 'all' || p.source === sourceFilter) &&
        (p.name.includes(q) || (p.description ?? '').toLowerCase().includes(q)),
    );
    count.textContent = `${visible.length} of ${packages.length}`;
    // Rendering ~1000 rows is slow and nobody scrolls that far; filter instead.
    list.replaceChildren(
      ...(visible.length ? visible.slice(0, 300).map((p) => row(p, load)) : [emptyState('Nothing matches')]),
    );
  };

  const load = async () => {
    list.replaceChildren(spinner());
    try {
      packages = await window.api.packages.installed();
      show();
    } catch (error) {
      list.replaceChildren(emptyState('Could not list packages', error.message));
    }
  };

  const tabs = ['all', 'repo', 'aur'].map((value) =>
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
      { all: 'All', repo: 'Official', aur: 'AUR' }[value],
    ),
  );

  container.append(
    h('header', { class: 'view-header' }, h('div', {}, h('h1', {}, 'Installed'), h('p', { class: 'subtitle' }, 'Packages you installed (dependencies are hidden)'))),
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

function row(pkg, refresh) {
  return h(
    'div',
    { class: 'row' },
    appIcon(pkg.icon, pkg.name),
    h(
      'div',
      { class: 'row-text' },
      h('div', { class: 'row-title' }, pkg.name, h('span', { class: 'version' }, pkg.version), pkg.source === 'aur' && h('span', { class: 'source aur' }, 'aur')),
      h('div', { class: 'row-desc' }, `${pkg.description} · ${pkg.size}`),
    ),
    packageButtons({ name: pkg.name, label: pkg.name, installed: true, hasApp: pkg.hasApp }, refresh),
  );
}

export default { id: 'installed', title: 'Installed', icon: 'installed', render };
