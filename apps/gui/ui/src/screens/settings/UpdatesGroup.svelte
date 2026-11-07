<!--
  Updates (inventory 5): idle / checking / upToDate / available / downloading
  (progress with ARIA progressbar, spring-eased fill) / error. The status
  region is aria-live=polite. Sizes in B / KiB / MiB / GiB via fmtBytes.
-->
<script lang="ts">
  import { scale } from 'svelte/transition';
  import { expoOut } from 'svelte/easing';
  import Download from 'lucide-svelte/icons/download';
  import CircleCheck from 'lucide-svelte/icons/circle-check';
  import Sparkles from 'lucide-svelte/icons/sparkles';
  import TriangleAlert from 'lucide-svelte/icons/triangle-alert';
  import RefreshCw from 'lucide-svelte/icons/refresh-cw';
  import InsetGroup from '../../components/ui/InsetGroup.svelte';
  import Button from '../../components/ui/Button.svelte';
  import Spinner from '../../components/ui/Spinner.svelte';
  import { check, type Update } from '../../lib/api';
  import { fmtBytes } from '../../lib/format';
  import { followSpring, reducedMotion, springs } from '../../lib/motion.svelte';

  type Status = 'idle' | 'checking' | 'available' | 'downloading' | 'upToDate' | 'error';
  let status = $state<Status>('idle');
  let updateVersion = $state('');
  let updateNotes = $state('');
  let updateError = $state('');
  let updateObj = $state.raw<Update | null>(null);
  let downloaded = $state(0);
  let total = $state(0);

  function humanErr(e: unknown): string {
    if (e == null) return 'Unknown error';
    if (typeof e === 'string') return e;
    if (e instanceof Error) return e.message;
    try {
      return JSON.stringify(e);
    } catch {
      return String(e);
    }
  }

  async function checkForUpdates() {
    status = 'checking';
    updateError = '';
    try {
      const update = await check();
      if (update) {
        updateVersion = update.version;
        updateNotes = update.body ?? '';
        updateObj = update;
        status = 'available';
      } else {
        status = 'upToDate';
      }
    } catch (e) {
      updateError = humanErr(e);
      status = 'error';
    }
  }

  async function downloadAndInstall() {
    if (!updateObj) return;
    status = 'downloading';
    downloaded = 0;
    total = 0;
    try {
      await updateObj.downloadAndInstall((event) => {
        switch (event.event) {
          case 'Started':
            total = event.data.contentLength ?? 0;
            break;
          case 'Progress':
            downloaded += event.data.chunkLength;
            break;
          case 'Finished':
            // The app is about to restart; stay in 'downloading'.
            break;
        }
      });
    } catch (e) {
      updateError = humanErr(e);
      status = 'error';
    }
  }

  const frac = $derived(total > 0 ? Math.min(1, downloaded / total) : 0);
  const pct = $derived(Math.round(frac * 100));
  const fill = followSpring(() => frac, springs.bar);

  const pop = $derived({ start: 0.6, duration: reducedMotion.current ? 0 : 280, easing: expoOut });
</script>

<InsetGroup title="Updates" id="updates">
  <div class="upd" aria-live="polite">
    <div class="status">
      <div class="orb {status}">
        {#key status}
          <span class="glyph" in:scale={pop}>
            {#if status === 'checking'}
              <Spinner size={18} />
            {:else if status === 'upToDate'}
              <CircleCheck size={20} strokeWidth={2} />
            {:else if status === 'available'}
              <Sparkles size={20} strokeWidth={2} />
            {:else if status === 'downloading'}
              <Download size={20} strokeWidth={2} />
            {:else if status === 'error'}
              <TriangleAlert size={20} strokeWidth={2} />
            {:else}
              <RefreshCw size={19} strokeWidth={2} />
            {/if}
          </span>
        {/key}
      </div>

      <div class="txt">
        {#if status === 'idle'}
          <p class="msg t-headline">Software Update</p>
        {:else if status === 'checking'}
          <p class="msg t-headline">Checking for updates...</p>
        {:else if status === 'upToDate'}
          <p class="msg t-headline ok">You're up to date!</p>
        {:else if status === 'available'}
          <p class="msg t-headline">Update available: v{updateVersion}</p>
          {#if updateNotes}<p class="sub t-callout">{updateNotes}</p>{/if}
        {:else if status === 'downloading'}
          <p class="msg t-headline num">
            {#if total > 0}
              Downloading: {fmtBytes(downloaded)} / {fmtBytes(total)} ({pct}%)
            {:else if downloaded > 0}
              Downloading: {fmtBytes(downloaded)}
            {:else}
              Preparing download...
            {/if}
          </p>
        {:else if status === 'error'}
          <p class="msg t-headline bad">{updateError}</p>
        {/if}
      </div>

      <div class="act">
        {#if status === 'idle'}
          <Button variant="primary" onclick={checkForUpdates}>Check for Updates</Button>
        {:else if status === 'upToDate'}
          <Button onclick={checkForUpdates}>Check Again</Button>
        {:else if status === 'available'}
          <Button variant="primary" onclick={downloadAndInstall}>Download &amp; Install</Button>
        {:else if status === 'error'}
          <Button onclick={checkForUpdates}>Try Again</Button>
        {/if}
      </div>
    </div>

    {#if status === 'downloading'}
      <div class="dl">
        {#if total > 0}
          <div
            class="track"
            role="progressbar"
            aria-label="Download progress"
            aria-valuemin="0"
            aria-valuemax={total}
            aria-valuenow={downloaded}
            aria-valuetext="{pct}%"
          >
            <div class="fill" style:transform="scaleX({Math.max(0, Math.min(1, fill.current))})"></div>
          </div>
        {:else}
          <div class="track" role="progressbar" aria-label="Download progress" aria-valuetext="Preparing">
            <div class="fill indet"></div>
          </div>
        {/if}
        <p class="note t-callout">The app will restart automatically when installation completes.</p>
      </div>
    {/if}
  </div>
</InsetGroup>

<style>
  .status {
    display: flex;
    align-items: center;
    gap: var(--sp-4);
    min-height: 72px;
    padding: var(--sp-4);
  }
  .orb {
    position: relative;
    flex: none;
    width: 40px;
    height: 40px;
    border-radius: 12px;
    display: grid;
    place-items: center;
    background: var(--accent-soft);
    color: var(--accent-strong);
    box-shadow: inset 0 0 0 1px var(--info-hair);
    transition:
      background-color var(--dur-med) ease-out,
      color var(--dur-med) ease-out,
      box-shadow var(--dur-med) ease-out;
  }
  .orb.checking {
    background: rgba(255, 255, 255, 0.07);
    color: var(--t2);
    box-shadow: inset 0 0 0 1px var(--hair);
  }
  .orb.upToDate {
    background: var(--success-surface);
    color: var(--success);
    box-shadow: inset 0 0 0 1px var(--success-hair);
  }
  .orb.error {
    background: var(--error-surface);
    color: var(--error);
    box-shadow: inset 0 0 0 1px var(--error-hair);
  }
  .glyph {
    display: grid;
    place-items: center;
  }
  .txt {
    flex: 1;
    min-width: 0;
  }
  .msg {
    color: var(--t1);
    overflow-wrap: anywhere;
  }
  .msg.ok {
    color: var(--success-text);
  }
  .msg.bad {
    color: var(--error-text);
    font-weight: 500;
  }
  .sub {
    color: var(--t3);
    margin-top: 2px;
    max-width: 62ch;
  }
  .act {
    flex: none;
    display: flex;
    gap: var(--sp-2);
  }

  .dl {
    padding: 0 var(--sp-4) var(--sp-4);
    box-shadow: none;
  }
  .track {
    height: 6px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.1);
    overflow: hidden;
    box-shadow: inset 0 1px 1px rgb(0 0 0 / 0.35);
  }
  .fill {
    height: 100%;
    width: 100%;
    border-radius: 999px;
    background: var(--accent);
    transform-origin: left center;
    box-shadow: inset 0 1px 0 rgb(255 255 255 / 0.35);
  }
  .fill.indet {
    width: 38%;
    animation: slide 1.35s var(--ease-out-expo) infinite;
  }
  @keyframes slide {
    from {
      transform: translateX(-110%);
    }
    to {
      transform: translateX(300%);
    }
  }
  .note {
    margin-top: var(--sp-3);
    color: var(--t3);
  }
  @media (prefers-reduced-motion: reduce) {
    .fill.indet {
      animation: none;
      width: 100%;
      opacity: 0.4;
    }
  }
</style>
