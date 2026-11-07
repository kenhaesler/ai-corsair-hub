<!--
  Global banners shown above content on every screen.
  Startup config-issue banner (inventory 0.3), structured like a macOS alert
  rather than a log line, with every required string kept verbatim:
    title · preserved / not-copied sentence · mono inset (path + error) ·
    "Running …" · "Saving any change …" · Dismiss (session) top-right.
-->
<script lang="ts">
  import TriangleAlert from 'lucide-svelte/icons/triangle-alert';
  import Button from '../ui/Button.svelte';
  import { ui } from '../../lib/stores/ui.svelte';

  const issue = $derived(ui.configIssueDismissed ? null : ui.configIssue);

  /** Split a Windows path into segments so it breaks after separators, never mid-name. */
  function pathParts(p: string): string[] {
    return p.split(/(?<=[\\/])/);
  }
  /** Split `backtick` spans out of an error message so they render as code, not literal backticks. */
  function errorParts(e: string): { code: boolean; text: string }[] {
    return e.split('`').map((text, i) => ({ code: i % 2 === 1, text }));
  }
</script>

{#if issue}
  <div class="global-banners">
    <div class="alert" role="alert" aria-labelledby="cfg-issue-title">
      <span class="icon" aria-hidden="true"><TriangleAlert size={18} strokeWidth={2.25} /></span>
      <div class="content">
        <div class="head">
          <h2 id="cfg-issue-title" class="title">Your configuration file could not be loaded.</h2>
          <Button size="sm" variant="secondary" onclick={() => (ui.configIssueDismissed = true)}>Dismiss</Button>
        </div>

        {#if issue.preserved_path}
          <p class="lead">It was preserved unchanged at</p>
        {:else}
          <p class="lead">
            It could NOT be copied to a backup{issue.preserve_error ? `: ${issue.preserve_error}` : ''}.
          </p>
        {/if}

        <div class="inset">
          {#if issue.preserved_path}
            <code class="path">{#each pathParts(issue.preserved_path) as seg, i (i)}{seg}<wbr />{/each}</code>
          {/if}
          <p class="err">
            <span class="err-label">Error:</span>
            <span class="err-text">{#each errorParts(issue.error) as part, i (i)}{#if part.code}<span class="tok">{part.text}</span>{:else}{part.text}{/if}{/each}.</span>
          </p>
        </div>

        <p class="body">
          {#if issue.loaded === 'last_good'}
            Running the last known good configuration instead.
          {:else}
            Running default settings instead — fan groups and RGB settings are not loaded.
          {/if}
        </p>
        <p class="body dim">Saving any change will write a new config.toml; the preserved file remains.</p>
      </div>
    </div>
  </div>
{/if}

<style>
  .global-banners {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-bottom: 24px;
  }
  .alert {
    display: flex;
    gap: 14px;
    padding: 16px 18px 16px 16px;
    border-radius: var(--r-row);
    background: linear-gradient(180deg, rgba(255, 159, 10, 0.1), rgba(255, 159, 10, 0.05));
    box-shadow:
      inset 0 0 0 1px var(--warn-hair),
      var(--hair-inner),
      var(--sh-1);
  }
  .icon {
    flex: none;
    width: 32px;
    height: 32px;
    border-radius: 9px;
    display: grid;
    place-items: center;
    background: rgba(255, 159, 10, 0.16);
    color: var(--warn);
  }
  .content {
    flex: 1;
    min-width: 0;
  }
  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    min-height: 26px;
  }
  .title {
    font-size: 14px;
    line-height: 20px;
    font-weight: 600;
    letter-spacing: -0.006em;
    color: var(--t1);
    padding-top: 3px;
  }
  .lead {
    margin-top: 2px;
    font-size: 13px;
    line-height: 18px;
    color: var(--warn-text);
  }
  .inset {
    margin: 10px 0 10px;
    padding: 10px 12px;
    border-radius: 10px;
    background: rgba(0, 0, 0, 0.32);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.06);
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 18px;
    color: var(--t1);
    max-width: 100ch;
  }
  .path {
    display: block;
    font-size: 12px;
    color: #ffe2b0;
    word-break: normal;
    overflow-wrap: normal;
  }
  .err {
    margin-top: 6px;
    color: var(--t2);
    white-space: normal;
    overflow-wrap: break-word;
  }
  .path + .err {
    padding-top: 6px;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.06);
  }
  .inset > .err:first-child {
    margin-top: 0;
  }
  .err-label {
    color: var(--warn-text);
    font-weight: 600;
    margin-right: 4px;
  }
  .err-text {
    color: var(--t1);
  }
  .tok {
    padding: 0 4px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.08);
    color: #ffe2b0;
  }
  .body {
    font-size: 13px;
    line-height: 18px;
    color: var(--warn-text);
  }
  .body.dim {
    margin-top: 2px;
    color: var(--t2);
  }
</style>
