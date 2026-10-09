// Maintenance: keep the system tidy — unused dependencies and the package cache.

import { h, runTask, spinner, emptyState, formatBytes, confirmDialog } from '../ui.js';

function render(container) {
  const body = h('div', { class: 'card-grid wide' });

  const load = async () => {
    body.replaceChildren(spinner());
    let info;
    try {
      info = await window.api.packages.systemInfo();
    } catch (error) {
      body.replaceChildren(emptyState('Could not read system info', error.message));
      return;
    }

    body.replaceChildren(
      tool({
        title: 'Unused dependencies',
        value: info.orphans.length === 0 ? 'None' : `${info.orphans.length} packages`,
        text:
          info.orphans.length === 0
            ? 'Nothing to clean up.'
            : `Installed as dependencies of apps that are gone: ${info.orphans.slice(0, 8).join(', ')}${info.orphans.length > 8 ? '…' : ''}`,
        action: info.orphans.length > 0 && 'Remove them',
        run: async () => {
          const ok = await confirmDialog({
            title: `Remove ${info.orphans.length} unused packages?`,
            message: 'Nothing you installed depends on them.',
            confirm: 'Remove',
            danger: true,
          });
          return ok && runTask('Removing unused dependencies', () => window.api.packages.removeOrphans(), { success: 'Unused packages removed' });
        },
      }),
      tool({
        title: 'Package cache',
        value: formatBytes(info.cacheBytes),
        text: 'Old package downloads kept by pacman. Cleaning keeps the two newest versions of each, so you can still roll back.',
        action: 'Clean cache',
        run: () => runTask('Cleaning package cache', () => window.api.packages.cleanCache(), { success: 'Package cache cleaned' }),
      }),
      tool({
        title: 'AUR helper',
        value: info.hasYay ? 'yay' : 'Missing',
        text: info.hasYay ? 'AUR packages can be installed and updated.' : 'Install yay to use packages from the AUR.',
      }),
    );
  };

  // Each tool card runs its action and reloads the numbers afterwards.
  const tool = ({ title, value, text, action, run }) =>
    h(
      'div',
      { class: 'card tool' },
      h('div', { class: 'tool-title' }, title),
      h('div', { class: 'tool-value' }, value),
      h('p', { class: 'card-desc' }, text),
      action &&
        h(
          'button',
          {
            class: 'small',
            onclick: async () => {
              if (await run()) load();
            },
          },
          action,
        ),
    );

  container.append(
    h('header', { class: 'view-header' }, h('div', {}, h('h1', {}, 'Maintenance'), h('p', { class: 'subtitle' }, 'Free up space and keep things tidy'))),
    body,
  );
  load();
}

export default { id: 'maintenance', title: 'Maintenance', icon: 'maintenance', render };
