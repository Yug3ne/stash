// Updates: pending updates for system packages (official + Omarchy repos),
// AUR packages and AppImages, in one place. AUR packages and AppImages can be
// updated one at a time; repo packages update together (see updatePackage).

import { h, runTask, spinner, emptyState, letterIcon, toast } from '../ui.js';
import { updatePackage, updateSystem } from './discover.js';

function setBadge(count) {
  document.dispatchEvent(new CustomEvent('badge', { detail: { id: 'updates', count } }));
}

/** Check everything at once. AppImage failures (e.g. offline) don't block packages. */
async function findUpdates(fresh = false) {
  const [packages, appimages] = await Promise.all([
    window.api.packages.checkUpdates(fresh),
    window.api.appimages.checkUpdates().catch(() => []),
  ]);
  setBadge(packages.length + appimages.length);
  return { packages, appimages };
}

function render(container) {
  const body = h('div');

  const load = async (fresh = false) => {
    body.replaceChildren(spinner('Checking for updates… (this takes a few seconds)'));
    try {
      show(await findUpdates(fresh));
    } catch (error) {
      body.replaceChildren(emptyState('Could not check for updates', error.message));
    }
  };

  const show = ({ packages, appimages }) => {
    const system = packages.filter((u) => u.source !== 'aur');
    const aur = packages.filter((u) => u.source === 'aur');
    if (packages.length === 0 && appimages.length === 0) {
      body.replaceChildren(emptyState('Everything is up to date', 'No package or AppImage updates right now.'));
      return;
    }

    const updateButton = (onclick) => h('button', { class: 'small', onclick }, 'Update');

    // h() skips false children; replaceChildren doesn't, hence the wrapper div.
    body.replaceChildren(
      h(
        'div',
        {},
        system.length > 0 &&
          panel({
            title: `System packages (${system.length})`,
            action: h('button', { class: 'primary', onclick: () => updateSystem(load) }, 'Update system'),
            note: window.isOmarchy
              ? 'Official Arch and Omarchy packages update together (Arch doesn’t support updating just some of them). Runs omarchy-update in a terminal, which takes a snapshot first and also updates AUR packages.'
              : 'Official packages update together (Arch doesn’t support updating just some of them). Opens a terminal.',
            rows: system.map((u) => updateRow(u)),
          }),
        aur.length > 0 &&
          panel({
            title: `AUR packages (${aur.length})`,
            action: h(
              'button',
              {
                class: 'primary',
                onclick: async () => {
                  toast('A terminal will open to rebuild the AUR packages');
                  if (await runTask('Updating AUR packages', () => window.api.packages.update(aur.map((u) => u.name)), { success: 'AUR packages updated' })) {
                    load();
                  }
                },
              },
              'Update all AUR',
            ),
            note: 'Each one is rebuilt from the AUR in a terminal. You can update them one at a time.',
            rows: aur.map((u) => updateRow(u, updateButton(() => updatePackage({ name: u.name, label: u.name, source: 'aur' }, load)))),
          }),
        appimages.length > 0 &&
          panel({
            title: `AppImages (${appimages.length})`,
            action: h(
              'button',
              {
                class: 'primary',
                onclick: async () => {
                  if (await runTask('Updating AppImages', () => window.api.appimages.updateAll(), { success: 'AppImages updated', showLog: true })) {
                    load();
                  }
                },
              },
              'Update all AppImages',
            ),
            rows: appimages.map((u) =>
              updateRow(
                { ...u, source: 'appimage' },
                updateButton(async () => {
                  if (await runTask(`Updating ${u.name}`, () => window.api.appimages.update(u.name), { success: `${u.name} updated`, showLog: true })) {
                    load();
                  }
                }),
              ),
            ),
          }),
      ),
    );
  };

  container.append(
    h(
      'header',
      { class: 'view-header' },
      h('div', {}, h('h1', {}, 'Updates'), h('p', { class: 'subtitle' }, 'Packages from Arch, Omarchy and the AUR, and your AppImages')),
      h('button', { class: 'ghost', onclick: () => load(true) }, 'Check again'),
    ),
    body,
  );
  load();
}

function panel({ title, action, note, rows }) {
  return h(
    'section',
    { class: 'panel' },
    h('div', { class: 'panel-header' }, h('h2', {}, title), action),
    note && h('p', { class: 'muted' }, note),
    h('div', { class: 'list compact' }, rows),
  );
}

function updateRow(update, button) {
  return h(
    'div',
    { class: 'row' },
    letterIcon(update.name),
    h(
      'div',
      { class: 'row-text' },
      h('div', { class: 'row-title' }, update.name, update.source !== 'repo' && h('span', { class: `source ${update.source}` }, update.source)),
      h('div', { class: 'row-desc mono' }, `${update.current} → ${update.latest}`),
    ),
    button,
  );
}

/** Called once at startup to fill the sidebar badge without opening the view. */
async function refreshBadge() {
  await findUpdates().catch(() => {});
}

export default { id: 'updates', title: 'Updates', icon: 'updates', render };
