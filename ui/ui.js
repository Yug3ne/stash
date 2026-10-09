// Small UI toolkit shared by all views: element builder, toasts, dialogs,
// formatting, and the activity panel that shows long-running tasks.

/**
 * Create an element. Children can be strings, elements, or arrays of them;
 * null/false children are skipped, which keeps conditionals tidy:
 *
 *   h('button', { class: 'primary', onclick: save }, 'Save')
 *   h('div', {}, isNew && h('span', { class: 'badge' }, 'New'))
 */
export function h(tag, props = {}, ...children) {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(props)) {
    if (value === undefined || value === null || value === false) continue;
    if (key.startsWith('on')) el.addEventListener(key.slice(2), value);
    else if (key === 'class') el.className = value;
    else if (key === 'style') el.style.cssText = value; // CSSOM: allowed by our CSP
    else if (key === 'html') el.innerHTML = value; // only for our own static SVG icons
    else if (key in el && typeof value !== 'string') el[key] = value;
    else el.setAttribute(key, value === true ? '' : value);
  }
  el.append(...children.flat(Infinity).filter((c) => c !== null && c !== undefined && c !== false));
  return el;
}

// ─── Formatting ──────────────────────────────────────────────────────────────

export function formatBytes(bytes) {
  if (!bytes) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  return `${(bytes / 1024 ** i).toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

/** Round avatar with the app's first letter, for apps without an icon. */
export function letterIcon(name) {
  let hue = 0;
  for (const char of name) hue = (hue * 31 + char.charCodeAt(0)) % 360;
  return h('div', { class: 'app-icon letter', style: `--hue: ${hue}` }, name.charAt(0).toUpperCase());
}

export function appIcon(src, name) {
  return src ? h('img', { class: 'app-icon', src, alt: '' }) : letterIcon(name);
}

export function emptyState(title, text) {
  return h('div', { class: 'empty' }, h('h3', {}, title), text && h('p', {}, text));
}

export function spinner(text = 'Loading…') {
  return h('div', { class: 'loading' }, h('span', { class: 'spinner' }), text);
}

// ─── Toasts ──────────────────────────────────────────────────────────────────

export function toast(message, type = 'info') {
  const el = h('div', { class: `toast ${type}` }, message);
  document.getElementById('toasts').append(el);
  setTimeout(() => el.classList.add('leaving'), type === 'error' ? 7000 : 3500);
  setTimeout(() => el.remove(), type === 'error' ? 7400 : 3900);
}

// ─── Dialog ──────────────────────────────────────────────────────────────────

/** Ask a yes/no question. Resolves true if confirmed. */
export function confirmDialog({ title, message, confirm = 'Continue', danger = false }) {
  return new Promise((resolve) => {
    const close = (answer) => {
      backdrop.remove();
      document.removeEventListener('keydown', onKey);
      resolve(answer);
    };
    const onKey = (event) => {
      if (event.key === 'Escape') close(false);
      if (event.key === 'Enter') close(true);
    };

    const backdrop = h(
      'div',
      { class: 'backdrop', onclick: (e) => e.target === backdrop && close(false) },
      h(
        'div',
        { class: 'dialog' },
        h('h3', {}, title),
        message && h('p', {}, message),
        h(
          'div',
          { class: 'dialog-actions' },
          h('button', { class: 'ghost', onclick: () => close(false) }, 'Cancel'),
          h('button', { class: danger ? 'danger' : 'primary', onclick: () => close(true) }, confirm),
        ),
      ),
    );
    document.body.append(backdrop);
    document.addEventListener('keydown', onKey);
    backdrop.querySelector('button:last-child').focus();
  });
}

// ─── Activity panel & tasks ──────────────────────────────────────────────────

const activity = {
  panel: () => document.getElementById('activity'),
  status: () => document.getElementById('activity-status'),
  log: () => document.getElementById('activity-log'),
};

let busy = false;
const busyListeners = new Set();

/** Views use this to disable buttons while something is running. */
export function isBusy() {
  return busy;
}

export function onBusyChange(listener) {
  busyListeners.add(listener);
  return () => busyListeners.delete(listener);
}

function setBusy(value) {
  busy = value;
  document.body.classList.toggle('busy', value);
  busyListeners.forEach((listener) => listener(value));
}

export function setupActivityPanel() {
  const toggle = document.getElementById('activity-toggle');
  const setOpen = (open) => {
    activity.panel().classList.toggle('collapsed', !open);
    toggle.textContent = open ? 'Hide log' : 'Show log';
  };
  toggle.addEventListener('click', () => setOpen(activity.panel().classList.contains('collapsed')));
  window.api.onTask(showTaskMessage);
  return setOpen;
}

const SYMBOLS = { step: '::', done: '✓', warn: '!' };

/** A progress message from Rust (see `Msg` in src/util.rs). */
function showTaskMessage({ kind, value }) {
  if (kind === 'progress') {
    // Download progress goes in the status line instead of flooding the log.
    const status = activity.status();
    status.textContent = `${status.dataset.label} — ${value}%`;
  } else {
    appendLog(`${SYMBOLS[kind] ?? ''} ${value}`);
  }
}

function appendLog(line) {
  const log = activity.log();
  const atBottom = log.scrollTop + log.clientHeight >= log.scrollHeight - 4;
  log.append(line + '\n');
  if (atBottom) log.scrollTop = log.scrollHeight;
}

/**
 * Run a long task with consistent feedback: one task at a time, status text in
 * the activity panel, a toast when it finishes, and errors shown to the user.
 *
 * Returns true if the task succeeded.
 */
export async function runTask(label, fn, { success, showLog = false } = {}) {
  if (busy) {
    toast('Please wait for the current task to finish', 'error');
    return false;
  }
  setBusy(true);
  activity.status().textContent = label;
  activity.status().dataset.label = label;
  activity.status().classList.add('running');
  appendLog(`\n── ${label} ──`);
  if (showLog) document.dispatchEvent(new CustomEvent('activity:open'));

  try {
    await fn();
    activity.status().textContent = `✓ ${success ?? label}`;
    if (success) toast(success, 'success');
    return true;
  } catch (error) {
    activity.status().textContent = `✗ ${label} failed`;
    appendLog(`✗ ${error.message}`);
    toast(error.message, 'error');
    return false;
  } finally {
    activity.status().classList.remove('running');
    setBusy(false);
  }
}
