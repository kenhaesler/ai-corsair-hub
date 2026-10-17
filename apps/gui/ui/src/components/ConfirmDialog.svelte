<script lang="ts">
  // Small reusable confirmation dialog for destructive actions.
  // Deliberately simpler than DevicePickerModal — no toolbar, no lists,
  // just a title, body, and confirm/cancel pair.

  interface Props {
    open: boolean;
    title: string;
    message: string;
    confirmLabel?: string;
    cancelLabel?: string;
    destructive?: boolean;
    onconfirm: () => void;
    oncancel: () => void;
  }

  let {
    open = $bindable(),
    title,
    message,
    confirmLabel = 'Confirm',
    cancelLabel = 'Cancel',
    destructive = false,
    onconfirm,
    oncancel,
  }: Props = $props();

  function handleBackdrop(e: MouseEvent) {
    if (e.target === e.currentTarget) oncancel();
  }
  function handleBackdropKey(e: KeyboardEvent) {
    if (e.target !== e.currentTarget) return;
    if (e.key === 'Escape' || e.key === 'Enter' || e.key === ' ') oncancel();
  }
  function handleKeydown(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === 'Escape') oncancel();
    if (e.key === 'Enter') onconfirm();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if open}
  <div
    class="backdrop"
    role="button"
    tabindex="-1"
    onclick={handleBackdrop}
    onkeydown={handleBackdropKey}
    aria-label={cancelLabel}
  >
    <div class="modal" role="dialog" aria-modal="true" aria-labelledby="confirm-title">
      <h3 id="confirm-title">{title}</h3>
      <p>{message}</p>
      <footer>
        <button class="cancel" onclick={oncancel}>{cancelLabel}</button>
        <button class="confirm" class:destructive onclick={onconfirm}>{confirmLabel}</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 120;
    padding: 24px;
    border: none;
    cursor: default;
  }
  .modal {
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.4);
    width: min(400px, 100%);
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  h3 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  p {
    margin: 0;
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.5;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  button {
    padding: 6px 14px;
    border-radius: var(--radius);
    cursor: pointer;
    font-size: 12px;
    font-family: inherit;
  }
  .cancel {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-secondary);
  }
  .cancel:hover {
    color: var(--text-primary);
    border-color: var(--text-muted);
  }
  .confirm {
    background: var(--accent);
    color: var(--bg-base);
    border: 1px solid var(--accent);
    font-weight: 600;
  }
  .confirm:hover {
    filter: brightness(1.1);
  }
  .confirm.destructive {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
  }
  .confirm.destructive:hover {
    filter: brightness(1.1);
  }
</style>
