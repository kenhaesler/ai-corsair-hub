<!--
  PageHost: keeps a page mounted (state preserved across tab switches) and
  cross-fades/slides it on a spring when its tab becomes active. Inactive
  pages are inert and visibility:hidden once the spring settles. Sets the
  PAGE_TAB context so ToolbarActions / LargeTitlePage know their tab.
-->
<script lang="ts">
  import { setContext, untrack, type Snippet } from 'svelte';
  import { Spring } from 'svelte/motion';
  import { ui, TABS, type Tab } from '../../lib/stores/ui.svelte';
  import { reducedMotion, springs } from '../../lib/motion.svelte';
  import { PAGE_TAB } from './context';

  interface Props {
    tab: Tab;
    children: Snippet;
  }
  let { tab, children }: Props = $props();
  // the tab of a PageHost never changes: read it once, untracked
  setContext(PAGE_TAB, untrack(() => tab));

  const a = new Spring(untrack(() => (ui.activeTab === tab ? 1 : 0)), { ...springs.page });
  const isActive = $derived(ui.activeTab === tab);
  $effect(() => {
    a.set(isActive ? 1 : 0, { instant: reducedMotion.current });
  });

  const dir = $derived(Math.sign(TABS.indexOf(tab) - TABS.indexOf(ui.activeTab)));
  const shown = $derived(isActive || a.current > 0.01);
</script>

<div
  class="page"
  id="page-{tab}"
  role="tabpanel"
  aria-labelledby="tab-{tab}"
  inert={!isActive}
  style:visibility={shown ? 'visible' : 'hidden'}
  style:opacity={a.current}
  style:transform={a.current > 0.999 || !shown ? 'none' : `translate3d(${dir * (1 - a.current) * 28}px, 0, 0)`}
  style:z-index={isActive ? 2 : 1}
>
  {@render children()}
</div>

<style>
  .page {
    position: absolute;
    inset: 0;
  }
</style>
