<!--
  Source list on top (inventory 2.1 group list): one capsule per group with
  name, mode (fixed / curve / pid), live duty % (display.snapshot, so it
  follows the scrub) and a status dot (green when live duty exists, else
  grey). A selection plate slides between capsules on a spring. "+ New Group"
  sits at the end (tooltip "Create a new fan group").
  The selected capsule is also the group's name editor (inventory 2.2): click
  its name (pencil, tooltip "Click to rename group") or press F2 to rename
  inline (max 64, Enter/blur commits, Escape cancels). Its mode label is
  hidden because the Mode control below is that datum's single home.
  Keyboard: tablist semantics, ←/→/Home/End, F2 rename.
-->
<script lang="ts">
  import { onMount, tick } from 'svelte';
  import Pencil from 'lucide-svelte/icons/pencil';
  import Plus from 'lucide-svelte/icons/plus';
  import CircleAlert from 'lucide-svelte/icons/circle-alert';
  import AnimatedNumber from '../../components/ui/AnimatedNumber.svelte';
  import { createSpring, springTo, springs } from '../../lib/motion.svelte';
  import { display } from '../../lib/stores/scrub.svelte';
  import { dutyOf, groupColor } from '../../lib/format';
  import type { FanGroupConfig } from '../../lib/types';
  import { MODE_LABEL } from './fanModes';

  interface Props {
    groups: FanGroupConfig[];
    selected: number;
    /** names with unsaved edits */
    edited: Set<string>;
    /** names with a hard curve error */
    invalid: Set<string>;
    panelId: string;
    onselect: (i: number) => void;
    onnew: () => void;
    onrename: (i: number, name: string) => void;
  }
  let { groups, selected, edited, invalid, panelId, onselect, onnew, onrename }: Props = $props();

  // ---- inline rename of the selected group
  let renaming = $state<number | null>(null);
  let draft = $state('');
  let input = $state<HTMLInputElement>();
  async function startRename(i: number) {
    if (i !== selected) return;
    draft = groups[i].name;
    renaming = i;
    await tick();
    input?.focus();
    input?.select();
  }
  function commitRename() {
    if (renaming === null) return;
    const i = renaming;
    const next = draft.trim();
    renaming = null;
    if (next && next !== groups[i]?.name) onrename(i, next);
    queueMicrotask(() => tabs[i]?.focus({ preventScroll: true }));
  }
  function cancelRename() {
    const i = renaming;
    renaming = null;
    draft = '';
    if (i !== null) queueMicrotask(() => tabs[i]?.focus({ preventScroll: true }));
  }
  $effect(() => {
    if (renaming !== null && renaming !== selected) renaming = null;
  });

  let row: HTMLDivElement;
  const tabs: HTMLDivElement[] = $state([]);
  const plate = createSpring({ x: 0, w: 0 }, springs.tab);
  let ready = $state(false);

  function measure(instant = false) {
    const b = tabs[selected];
    if (!b) return;
    springTo(plate, { x: b.offsetLeft, w: b.offsetWidth }, instant || !ready);
    ready = true;
    // keep the selected capsule in view
    const l = b.offsetLeft;
    const r = l + b.offsetWidth;
    if (l < row.scrollLeft) row.scrollTo({ left: l - 8, behavior: 'smooth' });
    else if (r > row.scrollLeft + row.clientWidth) row.scrollTo({ left: r - row.clientWidth + 8, behavior: 'smooth' });
  }
  $effect(() => {
    void selected;
    void groups.length;
    queueMicrotask(() => measure());
  });
  onMount(() => {
    measure(true);
    const ro = new ResizeObserver(() => measure(true));
    ro.observe(row);
    return () => ro.disconnect();
  });

  function onKey(e: KeyboardEvent) {
    if (renaming !== null) return;
    if (e.key === 'F2') {
      e.preventDefault();
      startRename(selected);
      return;
    }
    let j = selected;
    if (e.key === 'ArrowRight') j = Math.min(groups.length - 1, selected + 1);
    else if (e.key === 'ArrowLeft') j = Math.max(0, selected - 1);
    else if (e.key === 'Home') j = 0;
    else if (e.key === 'End') j = groups.length - 1;
    else return;
    e.preventDefault();
    onselect(j);
    tabs[j]?.focus();
  }
</script>

<div class="wrap">
  <div class="row" bind:this={row}>
    <div class="track" role="tablist" aria-label="Fan groups" tabindex="-1" onkeydown={onKey}>
      <span
        class="plate"
        class:ready
        style:transform="translateX({plate.current.x}px)"
        style:width="{plate.current.w}px"
        aria-hidden="true"
      ></span>
      {#each groups as g, i (i)}
        {@const duty = dutyOf(display.snapshot, g.name)}
        {@const c = groupColor(i)}
        {@const sel = i === selected}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          role="tab"
          class="capsule"
          class:sel
          aria-selected={sel}
          aria-controls={panelId}
          aria-keyshortcuts={sel ? 'F2' : undefined}
          aria-label="{g.name}, {MODE_LABEL[g.mode.type]} mode{duty !== null ? `, ${Math.round(duty)}% duty` : ', no live duty'}{sel ? '. Press F2 to rename' : ''}"
          tabindex={sel ? 0 : -1}
          bind:this={tabs[i]}
          onclick={() => onselect(i)}
          onkeydown={(e) => {
            if ((e.key === 'Enter' || e.key === ' ') && e.target === e.currentTarget) {
              e.preventDefault();
              onselect(i);
            }
          }}
          style:--c={c}
        >
          <span class="top">
            <span class="dot" class:live={duty !== null} aria-hidden="true"></span>
            {#if renaming === i}
              <input
                bind:this={input}
                class="name-input"
                bind:value={draft}
                maxlength="64"
                aria-label="Rename fan group"
                autocomplete="off"
                spellcheck="false"
                onclick={(e) => e.stopPropagation()}
                onblur={commitRename}
                onkeydown={(e) => {
                  e.stopPropagation();
                  if (e.key === 'Enter') {
                    e.preventDefault();
                    commitRename();
                  } else if (e.key === 'Escape') {
                    e.preventDefault();
                    cancelRename();
                  }
                }}
              />
            {:else if sel}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <span class="name renamable" title="Click to rename group" onclick={() => startRename(i)}>
                <span class="name-t">{g.name}</span>
                <Pencil size={12} strokeWidth={2.25} class="pencil" />
              </span>
            {:else}
              <span class="name"><span class="name-t">{g.name}</span></span>
            {/if}
            {#if invalid.has(g.name)}
              <span class="bad" title="Curve has an error"><CircleAlert size={13} strokeWidth={2.5} /></span>
            {:else if edited.has(g.name)}
              <span class="edited" title="Unsaved changes"></span>
            {/if}
            <span class="duty num" aria-hidden="true">
              {#if duty !== null}<AnimatedNumber value={duty} /><span class="pct">%</span>{:else}<span class="t3">—</span>{/if}
            </span>
          </span>
          <span class="bottom" aria-hidden="true">
            {#if !sel}<span class="mode">{MODE_LABEL[g.mode.type]}</span>{/if}
            <span class="meter"><span class="fill" style:transform="scaleX({(duty ?? 0) / 100})"></span></span>
          </span>
        </div>
      {/each}
    </div>
    <button type="button" class="new" title="Create a new fan group" onclick={onnew}>
      <Plus size={16} strokeWidth={2.25} />
      <span>New Group</span>
    </button>
  </div>
</div>

<style>
  .wrap {
    position: relative;
    margin: 0 calc(var(--gutter) * -1) 20px;
  }
  .row {
    display: flex;
    align-items: stretch;
    gap: 10px;
    overflow-x: auto;
    padding: 4px var(--gutter) 10px;
    scrollbar-width: none;
    mask-image: linear-gradient(to right, transparent 0, #000 var(--gutter), #000 calc(100% - var(--gutter)), transparent 100%);
  }
  .row::-webkit-scrollbar {
    display: none;
  }
  .track {
    position: relative;
    display: flex;
    gap: 10px;
    isolation: isolate;
  }
  .plate {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    z-index: -1;
    border-radius: var(--r-row);
    background: var(--mat-raised-hover);
    box-shadow:
      inset 0 0 0 1px var(--hair-strong),
      inset 0 1px 0 rgba(255, 255, 255, 0.08),
      var(--sh-2);
    opacity: 0;
  }
  .plate.ready {
    opacity: 1;
  }
  .capsule {
    position: relative;
    flex: none;
    width: 212px;
    height: 64px;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 7px;
    padding: 0 14px;
    border: 0;
    border-radius: var(--r-row);
    background: var(--mat-2);
    box-shadow:
      inset 0 0 0 1px var(--hair),
      var(--hair-inner);
    color: var(--t1);
    text-align: left;
    cursor: pointer;
    overflow: hidden;
    transition:
      background-color var(--dur-fast) ease-out,
      transform var(--dur-press) ease-out;
  }
  .capsule:hover:not(.sel) {
    background: var(--mat-2-hover);
  }
  .capsule:active {
    transform: scale(0.98);
  }
  .capsule.sel {
    background: transparent;
    box-shadow: none;
  }
  .capsule:focus-visible {
    box-shadow: var(--focus-ring);
  }
  .top {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .dot {
    flex: none;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.28);
  }
  .dot.live {
    background: var(--ok);
    box-shadow: 0 0 0 3px rgba(50, 215, 75, 0.16);
  }
  .name-input {
    flex: 1;
    min-width: 0;
    height: 24px;
    margin: -2px 0;
    padding: 0 6px;
    border: 0;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.32);
    box-shadow:
      inset 0 0 0 1px var(--accent),
      var(--focus-ring);
    color: var(--t1);
    font-size: 14px;
    font-weight: 600;
  }
  .name {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 14px;
    line-height: 20px;
    font-weight: 600;
    letter-spacing: -0.006em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name-t {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .renamable {
    cursor: text;
    border-radius: 6px;
  }
  .renamable :global(.pencil) {
    flex: none;
    color: var(--t3);
    transition: color var(--dur-fast) ease-out;
  }
  .renamable:hover :global(.pencil) {
    color: var(--t1);
  }
  .edited {
    flex: none;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
  }
  .bad {
    flex: none;
    display: grid;
    color: var(--error);
  }
  .bottom {
    display: flex;
    min-height: 16px;
    align-items: center;
    gap: 10px;
    padding-left: 15px;
  }
  .mode {
    flex: none;
    min-width: 34px;
    font-size: 12px;
    font-weight: 500;
    color: var(--t3);
  }
  .duty {
    flex: none;
    font-size: 17px;
    line-height: 20px;
    font-weight: 600;
    letter-spacing: -0.01em;
    color: var(--t1);
  }
  .pct {
    font-size: 12px;
    font-weight: 600;
    color: var(--t2);
    margin-left: 1px;
    letter-spacing: 0;
  }
  .meter {
    flex: 1;
    height: 3px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.07);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    background: var(--c);
    transform-origin: left center;
    transition: transform 600ms var(--ease-out-expo);
  }
  .new {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    height: 64px;
    padding: 0 18px;
    border: 0;
    border-radius: var(--r-row);
    background: transparent;
    outline: 1px dashed var(--hair-strong);
    outline-offset: -1px;
    color: var(--t2);
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition:
      background-color var(--dur-fast) ease-out,
      color var(--dur-fast) ease-out,
      transform var(--dur-press) ease-out;
  }
  .new:hover {
    background: var(--accent-softer);
    color: var(--accent-strong);
    outline-color: rgba(124, 156, 255, 0.45);
  }
  .new:active {
    transform: scale(0.97);
  }
  .new:focus-visible {
    box-shadow: var(--focus-ring);
  }
  @media (prefers-reduced-motion: reduce) {
    .fill {
      transition: none;
    }
  }
</style>
