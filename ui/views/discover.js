// Discover: a curated catalog of essential apps, plus search across the
// official Arch repos, Omarchy's own repository and the AUR.

import { h, runTask, toast, appIcon, spinner, emptyState, confirmDialog } from '../ui.js';

let searchTimer = null;
let lastQuery = '';

function render(container) {
  const input = h('input', {
    type: 'search',
    class: 'search',
    placeholder: 'Search Arch, Omarchy and AUR packages…',
    value: lastQuery,
    oninput: () => {
      // Wait until typing pauses so we don't search on every keystroke.
      clearTimeout(searchTimer);
      searchTimer = setTimeout(() => update(input.value), 300);
    },
  });
  const body = h('div', { class: 'discover-body' });

  container.append(
    h('header', { class: 'view-header' }, h('div', {}, h('h1', {}, 'Discover'), h('p', { class: 'subtitle' }, 'Essential apps for Omarchy, or search everything in Arch, the Omarchy repo and the AUR'))),
    input,
    body,
  );
  input.focus();

  const update = (query) => {
    lastQuery = query.trim();
    return lastQuery.length >= 2 ? showSearch(body, lastQuery) : showCatalog(body);
  };
  update(lastQuery);
}

// ─── Catalog ─────────────────────────────────────────────────────────────────

async function showCatalog(body) {
  body.replaceChildren(spinner());
  const catalog = await window.api.catalog();
  const names = catalog.flatMap((group) => group.apps.map((app) => app.pkg));
  const installed = await window.api.packages.status(names);

  if (lastQuery.length >= 2) return; // user started searching meanwhile
  body.replaceChildren(
    ...catalog.map((group) =>
      h(
        'section',
        { class: 'catalog-group' },
        h('h2', {}, group.category),
        h(
          'div',
          { class: 'card-grid' },
          group.apps.map((app) => catalogCard(app, installed[app.pkg], () => showCatalog(body))),
        ),
      ),
    ),
  );
}

function catalogCard(app, { installed, hasApp, icon }, refresh) {
  return h(
    'div',
    { class: 'card' },
    appIcon(icon, app.name),
    h('div', { class: 'card-text' }, h('div', { class: 'card-title' }, app.name), h('div', { class: 'card-desc' }, app.description)),
    packageButtons({ name: app.pkg, label: app.name, installed, hasApp }, refresh),
  );
}

// ─── Search ──────────────────────────────────────────────────────────────────

async function showSearch(body, query) {
  body.replaceChildren(spinner(`Searching for “${query}”…`));
  let results;
  try {
    results = await window.api.packages.search(query);
  } catch (error) {
    body.replaceChildren(emptyState('Search failed', error.message));
    return;
  }
  if (query !== lastQuery) return; // a newer search replaced this one

  if (results.length === 0) {
    body.replaceChildren(emptyState('No packages found', `Nothing in the Arch or Omarchy repos or the AUR matches “${query}”.`));
    return;
  }

  body.replaceChildren(
    h(
      'div',
      { class: 'list' },
      results.map((pkg) =>
        h(
          'div',
          { class: 'row' },
          appIcon(pkg.icon, pkg.name),
          h(
            'div',
            { class: 'row-text' },
            h(
              'div',
              { class: 'row-title' },
              pkg.name,
              h('span', { class: 'version' }, pkg.version),
              h('span', { class: `source ${pkg.source}` }, pkg.repo),
              pkg.outOfDate && h('span', { class: 'source warn' }, 'out of date'),
            ),
            h('div', { class: 'row-desc' }, pkg.description),
          ),
          pkg.source === 'aur' &&
            h('button', { class: 'ghost small', title: 'View on the AUR website', onclick: () => window.api.openUrl(`https://aur.archlinux.org/packages/${pkg.name}`) }, 'AUR ↗'),
          packageButtons({ name: pkg.name, label: pkg.name, installed: pkg.installed, hasApp: pkg.hasApp, aur: pkg.source === 'aur', source: pkg.source }, () => showSearch(body, query)),
        ),
      ),
    ),
  );
}

// ─── Shared install / open / update / remove buttons ─────────────────────────

/**
 * Buttons for one package. `source` is 'repo', 'omarchy' or 'aur'; `latest`
 * is the newer version when an update is available.
 */
function packageButtons({ name, label, installed, hasApp, aur, source, latest }, refresh) {
  if (!installed) {
    return h(
      'button',
      {
        class: 'primary small',
        onclick: async () => {
          if (aur) {
            const ok = await confirmDialog({
              title: `Install ${label} from the AUR?`,
              message: 'AUR packages are made by the community, not the Arch or Omarchy teams. Only install ones you trust.',
              confirm: 'Install',
            });
            if (!ok) return;
          }
          toast(`A terminal will open to install ${label} — enter your password there`);
          if (await runTask(`Installing ${label}`, () => window.api.packages.install([name]), { success: `${label} installed` })) {
            refresh();
          }
        },
      },
      'Install',
    );
  }

  return h(
    'div',
    { class: 'button-row' },
    hasApp && h('button', { class: 'small', onclick: () => window.api.packages.launch(name).catch((e) => toast(e.message, 'error')) }, 'Open'),
    latest && h('button', { class: 'primary small', onclick: () => updatePackage({ name, label, source }, refresh) }, 'Update'),
    h(
      'button',
      {
        class: 'ghost small danger-text',
        onclick: async () => {
          const ok = await confirmDialog({
            title: `Remove ${label}?`,
            message: 'The package and dependencies nothing else uses will be removed.',
            confirm: 'Remove',
            danger: true,
          });
          if (ok && (await runTask(`Removing ${label}`, () => window.api.packages.remove([name]), { success: `${label} removed` }))) {
            document.dispatchEvent(new Event('updates:changed'));
            refresh();
          }
        },
      },
      'Remove',
    ),
  );
}

/**
 * Update one package. AUR packages update on their own. Official and Omarchy
 * repo packages can't: Arch only supports upgrading them all together, so we
 * say so and offer the full system update instead.
 */
async function updatePackage({ name, label, source }, refresh) {
  if (source !== 'aur') {
    const where = source === 'omarchy' ? 'the Omarchy repository' : 'the official Arch repositories';
    const ok = await confirmDialog({
      title: `Update ${label}?`,
      message: `${label} comes from ${where}. Arch updates repository packages all together (updating just one can break other apps), so this runs a full system update${window.isOmarchy ? ' with omarchy-update, which takes a snapshot first' : ''}.`,
      confirm: 'Update system',
    });
    if (ok) await updateSystem(refresh);
    return;
  }
  toast(`A terminal will open to update ${label}`);
  if (await runTask(`Updating ${label}`, () => window.api.packages.update([name]), { success: `${label} updated` })) {
    document.dispatchEvent(new Event('updates:changed'));
    refresh();
  }
}

async function updateSystem(refresh) {
  if (await runTask('Updating system', () => window.api.packages.upgrade(), { success: 'System updated' })) {
    document.dispatchEvent(new Event('updates:changed'));
    refresh();
  }
}

export { packageButtons, updatePackage, updateSystem };
export default { id: 'discover', title: 'Discover', icon: 'discover', render };
