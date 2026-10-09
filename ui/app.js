// Page entry point: applies the theme, builds the sidebar, and swaps views.

import './api.js'; // defines window.api; must come first
import { h, setupActivityPanel, formatBytes } from './ui.js';
import icons from './icons.js';
import discover from './views/discover.js';
import appimages from './views/appimages.js';
import installed from './views/installed.js';
import updates from './views/updates.js';
import maintenance from './views/maintenance.js';

// Each view is { id, title, icon, render(container) }. Order = sidebar order.
// `icon` is a key in icons.js.
const views = [discover, appimages, installed, updates, maintenance];

function showView(id) {
  const view = views.find((v) => v.id === id) ?? views[0];
  for (const item of document.querySelectorAll('#nav li')) {
    item.classList.toggle('active', item.dataset.id === view.id);
  }
  const container = document.getElementById('view');
  container.replaceChildren();
  container.scrollTop = 0;
  view.render(container);
  try {
    localStorage.setItem('lastView', view.id);
  } catch {
    // Storage unavailable; just don't remember the view.
  }
}

function buildNav() {
  const nav = document.getElementById('nav');
  for (const view of views) {
    nav.append(
      h(
        'li',
        { 'data-id': view.id, onclick: () => showView(view.id) },
        h('span', { class: 'nav-icon', html: icons[view.icon] }),
        h('span', {}, view.title),
        h('span', { class: 'nav-badge', id: `badge-${view.id}` }),
      ),
    );
  }
}

/** Views call this (via a DOM event) to show counts like pending updates. */
document.addEventListener('badge', (event) => {
  const { id, count } = event.detail;
  const badge = document.getElementById(`badge-${id}`);
  if (badge) badge.textContent = count > 0 ? String(count) : '';
});

/** Something was updated: recount pending updates for the sidebar badge. */
document.addEventListener('updates:changed', () => updates.refreshBadge());

/** Views can ask to switch to another view. */
document.addEventListener('navigate', (event) => showView(event.detail));

/** Map the Omarchy theme's colors onto our CSS variables. */
async function applyTheme() {
  const colors = await window.api.theme().catch(() => null);
  if (!colors) return;
  const root = document.documentElement.style;
  const set = (variable, value) => value && root.setProperty(variable, value);
  set('--bg', colors.background);
  set('--bg-sidebar', colors.dark_background ?? colors.background);
  set('--surface', colors.lighter_background);
  set('--fg', colors.foreground);
  set('--muted', colors.muted ?? colors.dark_foreground);
  set('--accent', colors.accent);
  set('--selection', colors.selection);
  set('--green', colors.green);
  set('--red', colors.red);
  set('--yellow', colors.yellow);
  document.documentElement.dataset.mode = colors.mode ?? 'dark';
}

async function showSystemSummary() {
  const info = await window.api.packages.systemInfo().catch(() => null);
  if (!info) return;
  document
    .getElementById('system-summary')
    .replaceChildren(
      h('div', {}, `${info.totalCount} packages`),
      h('div', {}, `${info.explicitCount} installed by you`),
      h('div', {}, `Cache: ${formatBytes(info.cacheBytes)}`),
    );
}

async function start() {
  await applyTheme();
  // Re-read the theme when coming back to the window, in case it was changed.
  window.addEventListener('focus', applyTheme);

  buildNav();
  const openLog = setupActivityPanel();
  document.addEventListener('activity:open', () => openLog(true));

  let lastView = null;
  try {
    lastView = localStorage.getItem('lastView');
  } catch {
    // Ignore.
  }
  showView(lastView);
  showSystemSummary();

  // Refresh update counts in the background for the sidebar badge.
  updates.refreshBadge();
}

start();
