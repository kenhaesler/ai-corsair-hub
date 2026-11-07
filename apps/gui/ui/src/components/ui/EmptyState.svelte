<!--
  EmptyState: centred icon, title, message and optional action.
-->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { IconComponent } from '../../lib/icon';

  interface Props {
    icon?: IconComponent;
    title: string;
    message?: string;
    compact?: boolean;
    action?: Snippet;
  }
  let { icon: Icon, title, message, compact = false, action }: Props = $props();
</script>

<div class="empty" class:compact>
  {#if Icon}
    <span class="ic" aria-hidden="true"><Icon size={compact ? 18 : 22} strokeWidth={1.75} /></span>
  {/if}
  <p class="t-headline title">{title}</p>
  {#if message}<p class="t-callout msg">{message}</p>{/if}
  {#if action}<div class="act">{@render action()}</div>{/if}
</div>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 6px;
    padding: 40px 24px;
  }
  .compact {
    padding: 20px 16px;
  }
  .ic {
    width: 44px;
    height: 44px;
    border-radius: 14px;
    display: grid;
    place-items: center;
    background: rgba(255, 255, 255, 0.06);
    color: var(--t2);
    margin-bottom: 6px;
  }
  .compact .ic {
    width: 34px;
    height: 34px;
    border-radius: 10px;
  }
  .title {
    color: var(--t1);
  }
  .msg {
    color: var(--t3);
    max-width: 48ch;
  }
  .act {
    margin-top: 10px;
  }
</style>
