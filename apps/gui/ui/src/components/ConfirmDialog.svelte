<script lang="ts">
  import { tick } from 'svelte';
  // Small reusable confirmation dialog for destructive actions.
  // Deliberately simpler than DevicePickerModal — no toolbar, no lists,
  // just a title, body, and confirm/cancel pair.

  interface Props {
    open: boolean;
    title: string;
    message: string;
    confirmLabel?: string;
    cancelLabel?: string;
    secondaryLabel?: string;
    destructive?: boolean;
    onconfirm: () => void;
    oncancel: () => void;
    onsecondary?: () => void;
  }

  let {
    open = $bindable(),
    title,
    message,
    confirmLabel = 'Confirm',
    cancelLabel = 'Cancel',
    secondaryLabel,
    destructive = false,
    onconfirm,
    oncancel,
    onsecondary,
  }: Props = $props();

  let cancelButton = $state<HTMLButtonElement>();
  let confirmButton = $state<HTMLButtonElement>();
  let previousFocus: HTMLElement | null = null;
  let wasOpen = false;

  $effect(() => {
    if (open && !wasOpen) {
      previousFocus = document.activeElement as HTMLElement | null;
      void tick().then(() => {
        (destructive ? cancelButton : confirmButton)?.focus();
      });
    } else if (!open && wasOpen) {
      previousFocus?.focus();
      previousFocus = null;
    }
    wasOpen = open;
  });

  function handleBackdrop(event: MouseEvent) {
    if (event.target === event.currentTarget) oncancel();
  }

  function handleDialogKey(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      oncancel();
      return;
    }
    if (event.key !== 'Tab') return;

    const first = cancelButton;
    const last = confirmButton;
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last?.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first?.focus();
    }
  }
</script>

{#if open}
  <dialog
    open
    class="backdrop"
    aria-modal="true"
    aria-labelledby="confirm-title"
    onclick={handleBackdrop}
    onkeydown={handleDialogKey}
  >
    <div class="modal">
      <h3 id="confirm-title">{title}</h3>
      <p>{message}</p>
      <footer>
        <button bind:this={cancelButton} class="cancel" onclick={oncancel}>{cancelLabel}</button>
        {#if secondaryLabel && onsecondary}
          <button class="secondary" onclick={onsecondary}>{secondaryLabel}</button>
        {/if}
        <button bind:this={confirmButton} class="confirm" class:destructive onclick={onconfirm}>{confirmLabel}</button>
      </footer>
    </div>
  </dialog>
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
    width: 100%;
    height: 100%;
    max-width: none;
    max-height: none;
    margin: 0;
    color: var(--text-primary);
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
  .secondary {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-primary);
  }
  .secondary:hover {
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
