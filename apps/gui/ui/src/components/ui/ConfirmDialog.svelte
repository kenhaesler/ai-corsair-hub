<!--
  ConfirmDialog (inventory 0.5). Props open, title, message, confirmLabel
  ("Confirm"), cancelLabel ("Cancel"), destructive. Destructive gives a red
  confirm button. Escape cancels, Enter confirms, backdrop click cancels.
-->
<script lang="ts">
  import Button from './Button.svelte';
  import Sheet from './Sheet.svelte';
  import TriangleAlert from 'lucide-svelte/icons/triangle-alert';

  interface Props {
    open: boolean;
    title: string;
    message: string;
    confirmLabel?: string;
    cancelLabel?: string;
    destructive?: boolean;
    /** focus Cancel instead of the confirm button (for far-reaching actions) */
    focusCancel?: boolean;
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
    focusCancel = false,
    onconfirm,
    oncancel,
  }: Props = $props();

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      onconfirm();
    }
  }
  const tid = `cd-${Math.random().toString(36).slice(2, 8)}`;
</script>

<Sheet bind:open width={400} role="alertdialog" labelledby={tid} describedby="{tid}-msg" onclose={oncancel} onkeydown={onKey} closeButton={false}>
  {#snippet header()}
    <div class="hd">
      {#if destructive}
        <span class="icon" aria-hidden="true"><TriangleAlert size={20} strokeWidth={2} /></span>
      {/if}
      <h2 id={tid} class="t-title3">{title}</h2>
    </div>
  {/snippet}
  <p class="msg t-body" id="{tid}-msg">{message}</p>
  {#snippet footer()}
    <Button variant="secondary" onclick={oncancel} data-autofocus={focusCancel ? true : undefined}>{cancelLabel}</Button>
    <Button variant={destructive ? 'destructive' : 'primary'} onclick={onconfirm} data-autofocus={focusCancel ? undefined : true}>
      {confirmLabel}
    </Button>
  {/snippet}
</Sheet>

<style>
  .hd {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 24px 24px 0;
  }
  .icon {
    width: 40px;
    height: 40px;
    border-radius: 12px;
    display: grid;
    place-items: center;
    background: var(--error-surface);
    color: var(--error);
  }
  .msg {
    color: var(--t2);
  }
</style>
