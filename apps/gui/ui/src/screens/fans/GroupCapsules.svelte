<!--
  Group strip (inventory 2.1 group list), "technical luminous": one graphite
  panel of cells, one per group. Each cell: live dot + name, the live duty as
  a light figure glowing in its duty colour (display.snapshot, so it follows
  the scrub), the mode as a mono micro-label (hidden on the selected cell:
  the Mode control below is that datum's single home), and a luminous bar
  over the group's ghost operating envelope. A lit underline slides to the
  selected cell on a spring. "+ New group" is the last cell.
  The selected cell is also the group's name editor (inventory 2.2): click
  its name (pencil, tooltip "Click to rename group") or press F2 to rename
  inline (max 64, Enter/blur commits, Escape cancels).
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
  import { dutyOf, meanDuty } from '../../lib/format';
  import { dutyColor, groupEnvelope } from '../dashboard/overview';
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
    if (i < 0 || i !== selected || !groups[i]) return;
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
  let overflowing = $state(false);
  const tabs: HTMLDivElement[] = $state([]);
  let allEl = $state<HTMLDivElement>();

  // ---- "All fans": every group at once
  const allDuty = $derived(display.snapshot ? meanDuty(display.snapshot) : null);
  const allEnv = $derived.by((): [number, number] | null => {
    const envs = groups.map((g) => groupEnvelope(g)).filter((e): e is [number, number] => !!e);
    if (!envs.length) return null;
    return [Math.min(...envs.map((e) => e[0])), Math.max(...envs.map((e) => e[1]))];
  });
  const allMode = $derived.by(() => {
    const types = new Set(groups.map((g) => g.mode.type));
    return types.size === 1 ? MODE_LABEL[groups[0].mode.type] : 'Mixed';
  });
  const anyEdited = $derived(groups.some((g) => edited.has(g.name)));
  const anyInvalid = $derived(groups.some((g) => invalid.has(g.name)));
  const line = createSpring({ x: 0, w: 0 }, springs.tab);
  let ready = $state(false);

  function measure(instant = false) {
    if (row) overflowing = row.scrollWidth > row.clientWidth + 1;
    const b = selected < 0 ? allEl : tabs[selected];
    if (!b) return;
    springTo(line, { x: b.offsetLeft, w: b.offsetWidth }, instant || !ready);
    ready = true;
    // keep the selected cell in view
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
    else if (e.key === 'ArrowLeft') j = Math.max(-1, selected - 1);
    else if (e.key === 'Home') j = -1;
    else if (e.key === 'End') j = groups.length - 1;
    else return;
    e.preventDefault();
    onselect(j);
    (j < 0 ? allEl : tabs[j])?.focus();
  }
  const pct = (v: number) => `${Math.max(0, Math.min(100, v))}%`;
</script>

<div class="strip" class:overflowing>
  <div class="row" bind:this={row} onscroll={() => measure(true)}>
    <div class="track" role="tablist" aria-label="Fan groups" tabindex="-1" onkeydown={onKey}>
      <i class="uline" class:ready style:transform="translateX({line.current.x}px)" style:width="{line.current.w}px" aria-hidden="true"></i>
      {#if groups.length > 1}
        {@const sel = selected < 0}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          role="tab"
          class="cell all"
          class:sel
          aria-selected={sel}
          aria-controls={panelId}
          aria-label="All fans, {groups.length} groups, {allMode === 'Mixed' ? 'mixed modes' : `${allMode} mode`}{allDuty !== null ? `, ${Math.round(allDuty)}% average duty` : ''}. Changes here apply to every group"
          tabindex={sel ? 0 : -1}
          bind:this={allEl}
          onclick={() => onselect(-1)}
          onkeydown={(e) => {
            if ((e.key === 'Enter' || e.key === ' ') && e.target === e.currentTarget) {
              e.preventDefault();
              onselect(-1);
            }
          }}
          style:--c={dutyColor(allDuty ?? 0)}
        >
          <span class="top">
            <span class="dot" class:live={allDuty !== null} aria-hidden="true"></span>
            <span class="name"><span class="name-t">All fans</span></span>
            {#if anyInvalid}
              <span class="bad" title="A curve has an error"><CircleAlert size={13} strokeWidth={2.5} /></span>
            {:else if anyEdited}
              <span class="edited" title="Unsaved changes"></span>
            {/if}
          </span>
          <span class="mid" aria-hidden="true">
            <span class="duty">
              {#if allDuty !== null}<AnimatedNumber value={allDuty} /><small>%</small>{:else}<span class="nd">–</span>{/if}
            </span>
            {#if !sel}<span class="mode">{allMode}</span>{/if}
          </span>
          <span class="bar" aria-hidden="true">
            {#if allEnv}<i class="env" style:left={pct(allEnv[0])} style:width={pct(allEnv[1] - allEnv[0])}></i>{/if}
            <i class="fill" style:transform="scaleX({(allDuty ?? 0) / 100})"></i>
            {#if allDuty !== null}<i class="cap" style:left="calc({pct(allDuty)} - 1px)"></i>{/if}
          </span>
        </div>
      {/if}
      {#each groups as g, i (i)}
        {@const duty = dutyOf(display.snapshot, g.name)}
        {@const c = dutyColor(duty ?? 0)}
        {@const env = groupEnvelope(g)}
        {@const sel = i === selected}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          role="tab"
          class="cell"
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
          </span>
          <span class="mid" aria-hidden="true">
            <span class="duty">
              {#if duty !== null}<AnimatedNumber value={duty} /><small>%</small>{:else}<span class="nd">–</span>{/if}
            </span>
            {#if !sel}<span class="mode">{MODE_LABEL[g.mode.type]}</span>{/if}
          </span>
          <span class="bar" aria-hidden="true">
            {#if env}<i class="env" style:left={pct(env[0])} style:width={pct(env[1] - env[0])}></i>{/if}
            <i class="fill" style:transform="scaleX({(duty ?? 0) / 100})"></i>
            {#if duty !== null}<i class="cap" style:left="calc({pct(duty)} - 1px)"></i>{/if}
          </span>
        </div>
      {/each}
    </div>
    <button type="button" class="new" title="Create a new fan group" onclick={onnew}>
      <span class="plus"><Plus size={15} strokeWidth={2.25} /></span>
      <span>New group</span>
    </button>
  </div>
</div>

<style>
  .strip {
    margin-bottom: 20px;
    border-radius: 16px;
    background: var(--panel-grid), var(--panel);
    box-shadow:
      inset 0 0 0 1px var(--hair-strong),
      0 24px 48px -32px rgb(0 0 0 / 0.9);
    overflow: hidden;
  }
  .row {
    display: flex;
    align-items: stretch;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .row::-webkit-scrollbar {
    display: none;
  }
  .overflowing .new {
    box-shadow: -16px 0 24px -12px rgb(0 0 0 / 0.8);
  }
  .track {
    position: relative;
    display: flex;
    flex: 1 1 auto;
    isolation: isolate;
  }
  .uline {
    position: absolute;
    left: 0;
    bottom: 0;
    height: 2px;
    z-index: 1;
    border-radius: 2px 2px 0 0;
    background: linear-gradient(90deg, transparent 4%, color-mix(in oklab, var(--sc) 55%, #fff), transparent 96%);
    box-shadow:
      0 0 14px var(--sc),
      0 -8px 24px -6px color-mix(in oklab, var(--sc) 60%, transparent);
    opacity: 0;
    will-change: transform, width;
  }
  .uline.ready {
    opacity: 1;
  }
  .cell {
    position: relative;
    flex: 1 1 200px;
    min-width: 138px;
    container-type: inline-size;
    display: grid;
    gap: 10px;
    padding: 16px 18px 18px;
    border-left: 1px solid var(--hair);
    color: var(--t1);
    text-align: left;
    cursor: pointer;
    transition: background-color var(--dur-fast) ease-out;
  }
  .cell:first-child {
    border-left: 0;
  }
  .cell.all {
    flex: 0 1 196px;
    min-width: 124px;
    background: rgba(255, 255, 255, 0.015);
    border-right: 1px solid var(--hair-strong);
  }
  .cell.all .name-t {
    font-weight: 600;
  }
  .cell.all.sel {
    background: rgba(255, 255, 255, 0.045);
  }
  .cell:hover:not(.sel) {
    background: rgba(255, 255, 255, 0.025);
  }
  .cell.sel {
    background: rgba(255, 255, 255, 0.035);
  }
  .cell:focus-visible {
    box-shadow: inset var(--focus-ring);
    border-radius: 0;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    min-height: 20px;
  }
  .dot {
    flex: none;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.25);
  }
  .dot.live {
    background: #55e39a;
    box-shadow: 0 0 8px #55e39a;
  }
  .name-input {
    flex: 1;
    min-width: 0;
    height: 24px;
    margin: -2px 0;
    padding: 0 6px;
    border: 0;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.4);
    box-shadow:
      inset 0 0 0 1px var(--accent),
      var(--focus-ring);
    color: var(--t1);
    font-size: 14px;
    font-weight: 500;
  }
  .name {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 14.5px;
    line-height: 20px;
    font-weight: 500;
    letter-spacing: -0.006em;
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
    background: var(--warn);
    box-shadow: 0 0 6px var(--warn);
  }
  .bad {
    flex: none;
    display: grid;
    color: var(--error);
  }
  .mid {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
  }
  .duty {
    font-weight: 300;
    font-size: 30px;
    line-height: 1;
    letter-spacing: -0.035em;
    font-variant-numeric: tabular-nums;
    color: color-mix(in oklab, var(--c) 40%, #fff);
    text-shadow: 0 0 22px color-mix(in oklab, var(--c) 45%, transparent);
    transition: color 0.9s;
  }
  .duty small {
    font-size: 13px;
    color: var(--t3);
    margin-left: 3px;
    letter-spacing: 0;
    text-shadow: none;
  }
  .nd {
    color: var(--t3);
    text-shadow: none;
  }
  .mode {
    font: 500 10.5px var(--font-mono);
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--t3);
  }
  @container (max-width: 158px) {
    .mode {
      display: none;
    }
  }
  .bar {
    position: relative;
    display: block;
    height: 8px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.03);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.04);
  }
  .env {
    position: absolute;
    top: -2px;
    bottom: -2px;
    border: 1px solid rgba(255, 255, 255, 0.26);
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.025);
    transition:
      left 0.6s,
      width 0.6s;
  }
  .fill {
    position: absolute;
    inset: 0;
    transform-origin: left;
    border-radius: 2px;
    background: linear-gradient(
      90deg,
      color-mix(in oklab, var(--c) 18%, #15171c),
      color-mix(in oklab, var(--c) 75%, #1a1c22) 70%,
      color-mix(in oklab, var(--c) 40%, #fff)
    );
    box-shadow: 0 0 16px -3px color-mix(in oklab, var(--c) 70%, transparent);
    transition:
      transform 0.9s cubic-bezier(0.2, 0.8, 0.2, 1),
      background 0.9s;
  }
  .cap {
    position: absolute;
    top: -3px;
    bottom: -3px;
    width: 2px;
    border-radius: 2px;
    background: #fff;
    box-shadow: 0 0 8px 1px color-mix(in oklab, var(--c) 85%, #fff);
    transition: left 0.9s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .new {
    position: sticky;
    right: 0;
    z-index: 2;
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    width: 104px;
    padding: 0 12px;
    border: 0;
    border-left: 1px solid var(--hair);
    background: #0e1014;
    color: var(--t2);
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition:
      background-color var(--dur-fast) ease-out,
      color var(--dur-fast) ease-out;
  }
  .plus {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 9px;
    background: rgba(255, 255, 255, 0.05);
    box-shadow: inset 0 0 0 1px var(--hair-strong);
    transition: background-color var(--dur-fast) ease-out;
  }
  .new:hover {
    background: #13161b;
    color: var(--t1);
  }
  .new:hover .plus {
    background: var(--accent);
    color: var(--accent-ink);
  }
  .new:active .plus {
    transform: scale(0.95);
  }
  .new:focus-visible {
    box-shadow: inset 0 0 0 2px #08090b, inset 0 0 0 3px rgba(195, 209, 234, 0.7);
  }
  @media (prefers-reduced-motion: reduce) {
    .fill,
    .cap,
    .env {
      transition: none;
    }
  }
</style>
