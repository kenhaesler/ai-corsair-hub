// App-shell UI state: active tab, dashboard detail route, pinned highlights,
// per-page chrome (large-title collapse, back button) and the toolbar's
// trailing-actions registry.

import type { Snippet } from 'svelte';
import { flushSync, untrack } from 'svelte';
import type { StartupConfigIssue } from '../types';
import { reducedMotion } from '../motion.svelte';

export type Tab = 'dashboard' | 'fans' | 'devices' | 'lighting' | 'settings';
export const TABS: Tab[] = ['dashboard', 'fans', 'devices', 'lighting', 'settings'];

/** A highlight key: `temp:<source>` | `group:<name>` | `power:psu`. */
export type HighlightKey = string;

export interface PageChrome {
  /** title shown collapsed in the toolbar's leading slot */
  title: string;
  /** 0..1: how far the large title has scrolled under the toolbar */
  collapse: number;
  /** when set, the toolbar shows "‹ label" which calls onback */
  back: { label: string; onback: () => void } | null;
}

const PIN_KEY = 'health.pinned.v1';

interface PinState {
  order: HighlightKey[];
  hidden: HighlightKey[];
}

function loadPins(): PinState {
  try {
    const raw = localStorage.getItem(PIN_KEY);
    if (raw) {
      const v = JSON.parse(raw);
      if (Array.isArray(v.order) && Array.isArray(v.hidden)) return v;
    }
  } catch {
    /* storage unavailable: defaults */
  }
  return { order: [], hidden: [] };
}

class UiStore {
  activeTab = $state<Tab>('dashboard');
  /** Dashboard detail page: the highlight key being shown, or null for the Summary. */
  detail = $state<HighlightKey | null>(null);
  /** Highlight currently carrying view-transition-names. */
  morphKey = $state<HighlightKey | null>(null);
  /** Edit mode for pinned highlights. */
  editingPins = $state(false);
  pins = $state<PinState>(loadPins());
  /** Fan Control should select this group when it next renders. */
  fanFocusGroup = $state<string | null>(null);
  /** Name of the group selected in Fan Control (the status bar hides what its editor already shows). */
  fanSelectedGroup = $state<string | null>(null);
  /** Which readings tab the Summary's spec panel shows (the Control path hides values that tab already shows). */
  specTab = $state<'temps' | 'groups' | 'power' | 'speeds'>('temps');
  /** Element that opened the current Detail page; focus returns to it on close. */
  detailOpener: HTMLElement | null = null;

  chrome = $state<Record<Tab, PageChrome>>({
    dashboard: { title: 'Summary', collapse: 0, back: null },
    fans: { title: 'Fan Control', collapse: 0, back: null },
    devices: { title: 'Devices', collapse: 0, back: null },
    lighting: { title: 'Lighting', collapse: 0, back: null },
    settings: { title: 'Settings', collapse: 0, back: null },
  });

  /** Trailing toolbar actions per tab (see components/shell/ToolbarActions.svelte). */
  actions = $state.raw<Record<Tab, Snippet | null>>({
    dashboard: null,
    fans: null,
    devices: null,
    lighting: null,
    settings: null,
  });

  /** Startup config issue (inventory 0.3) and its session dismissal. */
  configIssue = $state<StartupConfigIssue | null>(null);
  configIssueDismissed = $state(false);

  setActions(tab: Tab, s: Snippet | null) {
    // untracked read: callers run this inside effects
    const cur = untrack(() => this.actions);
    if (cur[tab] === s) return;
    this.actions = { ...cur, [tab]: s };
  }

  persistPins() {
    try {
      localStorage.setItem(PIN_KEY, JSON.stringify($state.snapshot(this.pins)));
    } catch {
      /* ignore */
    }
  }

  /** Order the available keys by the user's pin order; hidden ones excluded unless `includeHidden`. */
  orderHighlights(available: HighlightKey[], includeHidden = false): HighlightKey[] {
    const known = this.pins.order.filter((k) => available.includes(k));
    const fresh = available.filter((k) => !this.pins.order.includes(k));
    const all = [...known, ...fresh];
    return includeHidden ? all : all.filter((k) => !this.pins.hidden.includes(k));
  }

  togglePin(key: HighlightKey, available: HighlightKey[]) {
    this.pins.order = this.orderHighlights(available, true);
    this.pins.hidden = this.pins.hidden.includes(key)
      ? this.pins.hidden.filter((k) => k !== key)
      : [...this.pins.hidden, key];
    this.persistPins();
  }

  movePin(key: HighlightKey, delta: number, available: HighlightKey[]) {
    const order = this.orderHighlights(available, true);
    const i = order.indexOf(key);
    const j = i + delta;
    if (i < 0 || j < 0 || j >= order.length) return;
    [order[i], order[j]] = [order[j], order[i]];
    this.pins.order = order;
    this.persistPins();
  }

  /**
   * Navigate the dashboard to a highlight detail (or back with null), using
   * a View Transition shared-element morph when available.
   */
  openDetail(key: HighlightKey | null) {
    const from = this.detail;
    const morph = key ?? from;
    const update = () => {
      this.detail = key;
      flushSync();
    };
    const doc = document as Document & { startViewTransition?: (cb: () => void) => { finished: Promise<void> } };
    if (!doc.startViewTransition || reducedMotion.current) {
      this.morphKey = null;
      this.detail = key;
      return;
    }
    this.morphKey = morph;
    flushSync();
    const vt = doc.startViewTransition(update);
    vt.finished.finally(() => {
      if (this.morphKey === morph) this.morphKey = null;
    });
  }

  goTab(tab: Tab) {
    this.activeTab = tab;
  }
}

export const ui = new UiStore();

// Stores hold live listeners and buffers; a hot swap of this module would
// leave the app wired to the old instance. Reload the page instead.
if (import.meta.hot) import.meta.hot.accept(() => window.location.reload());
