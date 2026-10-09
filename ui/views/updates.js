// Updates: pending updates for system packages (repo + AUR) and AppImages,
// in one place.

import { h, runTask, spinner, emptyState, letterIcon } from '../ui.js';

function setBadge(count) {
  document.dispatchEvent(new CustomEvent('badge', { detail: { id: 'updates', count } }));
}

/** Check everything at once. AppImage failures (e.g. offline) don't block packages. */
async function findUpdates() {
  const [packages, appimages] = await Promise.all([
    window.api.packages.checkUpdates(),
    window.api.appimages.checkUpdates().catch(() => []),
  ]);
  setBadge(packages.length + appimages.length);
  return { packages, appimages };
}

function render(container) {
  const body = h('div');

  const load = async () => {
    body.replaceChildren(spinner('Checking for updates… (this takes a few seconds)'));
    try {
      show(await findUpdates());
    } catch (error) {
      body.replaceChildren(emptyState('Could not check for updates', error.message));
    }
  };

  const show = ({ packages, appimages }) => {
    if (packages.length === 0 && appimages.length === 0) {
      body.replaceChildren(emptyState('Everything is up to date', 'No package or AppImage updates right now.'));
      return;
    }
    // h() skips false children; replaceChildren doesn't, hence the wrapper div.
    body.replaceChildren(
      h(
        'div',
        {},
        packages.length > 0 &&
          h(
            'section',
            { class: 'panel' },
            h(
              'div',
              { class: 'panel-header' },
              h('h2', {}, `System packages (${packages.length})`),
              h(
                'button',
                {
                  class: 'primary',
                  onclick: async () => {
                    if (
                      await runTask('Updating system', () => window.api.packages.upgrade(), {
                        success: 'System updated',
                      })
                    )
                      load();
                  },
                },
                'Update system',
              ),
            ),
            h(
              'p',
              { class: 'muted' },
              'Opens a terminal. On Omarchy this runs omarchy-update, which also takes a snapshot first.',
            ),
            h('div', { class: 'list compact' }, packages.map(updateRow)),
          ),
        appimages.length > 0 &&
          h(
            'section',
            { class: 'panel' },
            h(
              'div',
              { class: 'panel-header' },
              h('h2', {}, `AppImages (${appimages.length})`),
              h(
                'button',
                {
                  class: 'primary',
                  onclick: async () => {
                    if (
                      await runTask('Updating AppImages', () => window.api.appimages.updateAll(), {
                        success: 'AppImages updated',
                        showLog: true,
                      })
                    )
                      load();
                  },
                },
                'Update AppImages',
              ),
            ),
            h(
              'div',
              { class: 'list compact' },
              appimages.map((u) => updateRow({ ...u, source: 'appimage' })),
            ),
          ),
      ),
    );
  };

  container.append(
    h(
      'header',
      { class: 'view-header' },
      h(
        'div',
        {},
        h('h1', {}, 'Updates'),
        h('p', { class: 'subtitle' }, 'Packages from Arch, the AUR, and your AppImages'),
      ),
      h('button', { class: 'ghost', onclick: load }, 'Check again'),
    ),
    body,
  );
  load();
}

function updateRow(update) {
  return h(
    'div',
    { class: 'row' },
    letterIcon(update.name),
    h(
      'div',
      { class: 'row-text' },
      h('div', { class: 'row-title' }, update.name, h('span', { class: `source ${update.source}` }, update.source)),
      h('div', { class: 'row-desc mono' }, `${update.current} → ${update.latest}`),
    ),
  );
}

/** Called once at startup to fill the sidebar badge without opening the view. */
async function refreshBadge() {
  await findUpdates().catch(() => {});
}

export default { id: 'updates', title: 'Updates', icon: 'updates', render, refreshBadge };
