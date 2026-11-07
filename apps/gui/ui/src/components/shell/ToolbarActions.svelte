<!--
  ToolbarActions: register content for the toolbar's trailing slot while
  this page (tab) is mounted. Use anywhere inside a screen:

    <ToolbarActions>
      <Button variant="primary" onclick={save}>Save Configuration</Button>
    </ToolbarActions>

  The tab is read from the page context set by App (PAGE_TAB), or pass `tab`.
  Registering again replaces the previous snippet for that tab; the slot is
  cleared on unmount. Pass `active={false}` to hide without unmounting.
-->
<script lang="ts">
  import { getContext, untrack, type Snippet } from 'svelte';
  import { ui, type Tab } from '../../lib/stores/ui.svelte';
  import { PAGE_TAB } from './context';

  interface Props {
    tab?: Tab;
    active?: boolean;
    children: Snippet;
  }
  let { tab, active = true, children }: Props = $props();
  const ctxTab = getContext<Tab | undefined>(PAGE_TAB);
  const target = $derived(tab ?? ctxTab ?? 'dashboard');

  $effect(() => {
    const t = target;
    if (!active) return;
    ui.setActions(t, children);
    return () => {
      if (untrack(() => ui.actions[t]) === children) ui.setActions(t, null);
    };
  });
</script>
