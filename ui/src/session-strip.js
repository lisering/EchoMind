/**
 * Session Strip UI module (REQ-RAG-046)
 *
 * Provides a modal panel for stripping (removing) messages from a conversation.
 * Supports two modes:
 * 1. Range strip: remove messages by index range [from, to]
 * 2. Keep recent: keep only the last N messages
 *
 * Optionally inserts a summary system message to replace stripped messages.
 */
import { convApi } from './ipc.js';
import { showConfirmDialog } from './confirm-dialog.js';
import { getState } from './state.js';

let stripPanel = null;
let currentConversationId = null;

/**
 * Open the strip panel for a conversation.
 * @param {string} conversationId - The conversation ID
 */
export async function openStripPanel(conversationId) {
  currentConversationId = conversationId;
  const messages = await convApi.messages(conversationId);

  if (!messages || messages.length === 0) {
    return;
  }

  // Create panel if not exists
  if (!stripPanel) {
    createStripPanel();
  }

  // Update message count info
  const countEl = stripPanel.querySelector('#strip-msg-count');
  if (countEl) {
    countEl.textContent = messages.length;
  }

  // Reset inputs
  const fromInput = stripPanel.querySelector('#strip-from');
  const toInput = stripPanel.querySelector('#strip-to');
  const keepNInput = stripPanel.querySelector('#strip-keep-n');
  if (fromInput) {
    fromInput.value = '0';
    fromInput.max = Math.max(0, messages.length - 1);
  }
  if (toInput) {
    toInput.value = String(Math.max(0, messages.length - 1));
    toInput.max = String(Math.max(0, messages.length - 1));
  }
  if (keepNInput) {
    keepNInput.value = '3';
    keepNInput.max = String(messages.length);
  }

  stripPanel.classList.remove('hidden');
}

/**
 * Close the strip panel.
 */
export function closeStripPanel() {
  if (stripPanel) {
    stripPanel.classList.add('hidden');
  }
  currentConversationId = null;
}

/**
 * Create the strip panel DOM.
 */
function createStripPanel() {
  stripPanel = document.createElement('div');
  stripPanel.id = 'strip-panel';
  stripPanel.className = 'fixed inset-0 z-50 hidden flex items-center justify-center bg-black/50';
  stripPanel.innerHTML = `
    <div class="bg-surface-1 border border-line rounded-xl shadow-xl w-full max-w-lg mx-4 p-6 space-y-4">
      <div class="flex items-center justify-between">
        <h2 class="text-lg font-semibold text-text-primary">Session Strip</h2>
        <button id="strip-close" class="text-text-tertiary hover:text-text-primary text-xl">&times;</button>
      </div>
      <p class="text-sm text-text-secondary">
        <span id="strip-msg-count">0</span> messages in this conversation. Stripping removes messages to reduce context.
      </p>

      <!-- Mode tabs -->
      <div class="flex gap-2 border-b border-line">
        <button id="strip-tab-range" class="px-4 py-2 text-sm font-medium text-accent border-b-2 border-accent">Range</button>
        <button id="strip-tab-recent" class="px-4 py-2 text-sm font-medium text-text-tertiary border-b-2 border-transparent">Keep Recent</button>
      </div>

      <!-- Range mode -->
      <div id="strip-range-mode" class="space-y-3">
        <div class="flex gap-3 items-center">
          <label class="text-sm text-text-secondary">From</label>
          <input id="strip-from" type="number" min="0" value="0" class="flex-1 bg-surface-2 border border-line rounded px-2 py-1 text-sm text-text-primary">
          <label class="text-sm text-text-secondary">To</label>
          <input id="strip-to" type="number" min="0" value="0" class="flex-1 bg-surface-2 border border-line rounded px-2 py-1 text-sm text-text-primary">
        </div>
        <button id="strip-preview-btn" class="w-full px-3 py-2 text-sm bg-surface-2 hover:bg-surface-3 border border-line rounded text-text-primary">Preview</button>
        <div id="strip-preview-result" class="hidden text-sm text-text-secondary bg-surface-2 rounded p-3"></div>
      </div>

      <!-- Keep recent mode -->
      <div id="strip-recent-mode" class="hidden space-y-3">
        <div class="flex gap-3 items-center">
          <label class="text-sm text-text-secondary">Keep last</label>
          <input id="strip-keep-n" type="number" min="0" value="3" class="flex-1 bg-surface-2 border border-line rounded px-2 py-1 text-sm text-text-primary">
          <span class="text-sm text-text-tertiary">messages</span>
        </div>
      </div>

      <!-- Summary option -->
      <div class="space-y-2">
        <label class="flex items-center gap-2 text-sm text-text-secondary">
          <input id="strip-summary-check" type="checkbox" class="accent-blue-500">
          Replace with summary
        </label>
        <textarea id="strip-summary-text" class="hidden w-full bg-surface-2 border border-line rounded px-2 py-1 text-sm text-text-primary" rows="2" placeholder="Summary text..."></textarea>
      </div>

      <!-- Execute -->
      <button id="strip-execute-btn" class="w-full px-3 py-2 text-sm bg-red-600 hover:bg-red-700 text-white rounded font-medium">Strip Messages</button>
    </div>
  `;
  document.body.appendChild(stripPanel);

  // Wire up events
  stripPanel.querySelector('#strip-close').addEventListener('click', closeStripPanel);
  stripPanel.addEventListener('click', (e) => {
    if (e.target === stripPanel) closeStripPanel();
  });

  // Tab switching
  const tabRange = stripPanel.querySelector('#strip-tab-range');
  const tabRecent = stripPanel.querySelector('#strip-tab-recent');
  const rangeMode = stripPanel.querySelector('#strip-range-mode');
  const recentMode = stripPanel.querySelector('#strip-recent-mode');

  tabRange.addEventListener('click', () => {
    tabRange.classList.add('text-accent', 'border-accent');
    tabRange.classList.remove('text-text-tertiary', 'border-transparent');
    tabRecent.classList.add('text-text-tertiary', 'border-transparent');
    tabRecent.classList.remove('text-accent', 'border-accent');
    rangeMode.classList.remove('hidden');
    recentMode.classList.add('hidden');
  });

  tabRecent.addEventListener('click', () => {
    tabRecent.classList.add('text-accent', 'border-accent');
    tabRecent.classList.remove('text-text-tertiary', 'border-transparent');
    tabRange.classList.add('text-text-tertiary', 'border-transparent');
    tabRange.classList.remove('text-accent', 'border-accent');
    recentMode.classList.remove('hidden');
    rangeMode.classList.add('hidden');
  });

  // Summary checkbox
  const summaryCheck = stripPanel.querySelector('#strip-summary-check');
  const summaryText = stripPanel.querySelector('#strip-summary-text');
  summaryCheck.addEventListener('change', () => {
    summaryText.classList.toggle('hidden', !summaryCheck.checked);
  });

  // Preview button
  stripPanel.querySelector('#strip-preview-btn').addEventListener('click', previewStrip);

  // Execute button
  stripPanel.querySelector('#strip-execute-btn').addEventListener('click', executeStrip);
}

/**
 * Preview the strip operation.
 */
async function previewStrip() {
  if (!currentConversationId) return;
  const fromIdx = parseInt(stripPanel.querySelector('#strip-from').value, 10) || 0;
  const toIdx = parseInt(stripPanel.querySelector('#strip-to').value, 10) || 0;

  try {
    const result = await convApi.previewStrip(currentConversationId, fromIdx, toIdx);
    const previewEl = stripPanel.querySelector('#strip-preview-result');
    if (result && result.messages && result.messages.length > 0) {
      const roles = result.messages.map(m => m.role).join(', ');
      previewEl.innerHTML = `Will strip <strong>${result.messages.length}</strong> messages (${roles}). Est. ${result.estimated_tokens_saved || 0} tokens saved.`;
    } else {
      previewEl.innerHTML = 'No messages will be stripped.';
    }
    previewEl.classList.remove('hidden');
  } catch (err) {
    const previewEl = stripPanel.querySelector('#strip-preview-result');
    previewEl.textContent = `Error: ${err}`;
    previewEl.classList.remove('hidden');
  }
}

/**
 * Execute the strip operation.
 */
async function executeStrip() {
  if (!currentConversationId) return;

  const isRecentMode = !stripPanel.querySelector('#strip-recent-mode').classList.contains('hidden');
  const replaceWithSummary = stripPanel.querySelector('#strip-summary-check').checked;
  const summaryText = replaceWithSummary ? stripPanel.querySelector('#strip-summary-text').value : null;

  const confirmed = await showConfirmDialog({
    title: 'Confirm Strip',
    body: 'Are you sure you want to strip messages? This cannot be undone.',
    confirmText: 'Strip',
    cancelText: 'Cancel',
    danger: true,
  });
  if (!confirmed) return;

  try {
    let result;
    if (isRecentMode) {
      const keepN = parseInt(stripPanel.querySelector('#strip-keep-n').value, 10) || 0;
      result = await convApi.stripKeepingRecent(currentConversationId, keepN, replaceWithSummary, summaryText);
    } else {
      const fromIdx = parseInt(stripPanel.querySelector('#strip-from').value, 10) || 0;
      const toIdx = parseInt(stripPanel.querySelector('#strip-to').value, 10) || 0;
      result = await convApi.stripMessages(currentConversationId, fromIdx, toIdx, replaceWithSummary, summaryText);
    }

    closeStripPanel();

    // Reload messages by dispatching a custom event
    window.dispatchEvent(new CustomEvent('echomind:reload-messages', {
      detail: { conversationId: currentConversationId }
    }));
  } catch (err) {
    // Show error
  }
}

/**
 * Initialize strip functionality.
 */
export function initSessionStrip() {
  // Add strip button to conversation header
  const header = document.querySelector('#conversation-header');
  if (header && !header.querySelector('#strip-btn')) {
    const btn = document.createElement('button');
    btn.id = 'strip-btn';
    btn.className = 'p-1.5 text-text-tertiary hover:text-text-primary rounded-lg transition-colors';
    btn.title = 'Strip messages';
    btn.innerHTML = '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/></svg>';
    btn.addEventListener('click', async () => {
      const state = getState();
      if (state && state.currentConversationId) {
        await openStripPanel(state.currentConversationId);
      }
    });
    header.appendChild(btn);
  }
}
