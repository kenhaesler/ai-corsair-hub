<!--
  Settings (inventory 5): System-Settings-style grouped form. General (poll
  interval, log level, Save Settings), Updates, About. Edits live in
  configStore.config and persist only through Save Settings.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import Gauge from 'lucide-svelte/icons/gauge';
  import ScrollText from 'lucide-svelte/icons/scroll-text';
  import Cpu from 'lucide-svelte/icons/cpu';
  import LargeTitlePage from '../../components/shell/LargeTitlePage.svelte';
  import Banner from '../../components/ui/Banner.svelte';
  import Button from '../../components/ui/Button.svelte';
  import InsetGroup from '../../components/ui/InsetGroup.svelte';
  import InsetRow from '../../components/ui/InsetRow.svelte';
  import Select from '../../components/ui/Select.svelte';
  import Skeleton from '../../components/ui/Skeleton.svelte';
  import Slider from '../../components/ui/Slider.svelte';
  import { getVersion } from '../../lib/api';
  import { configStore, loadConfig, saveCurrentConfig } from '../../lib/stores/config.svelte';
  import AppMark from './AppMark.svelte';
  import UpdatesGroup from './UpdatesGroup.svelte';

  let appVersion = $state('');

  onMount(() => {
    loadConfig();
    getVersion()
      .then((v) => (appVersion = v))
      .catch(() => {
        /* version unavailable: leave blank */
      });
  });

  const LOG_LEVELS = [
    { value: 'error', label: 'Error' },
    { value: 'warn', label: 'Warn' },
    { value: 'info', label: 'Info' },
    { value: 'debug', label: 'Debug' },
    { value: 'trace', label: 'Trace' },
  ];
</script>

<LargeTitlePage title="Settings" maxWidth={800}>
  {#if configStore.error}
    <div class="banner"><Banner tone="error">{configStore.error}</Banner></div>
  {/if}

  <div class="groups">
    <InsetGroup title="General" id="general">
      {#if configStore.config}
        {@const general = configStore.config.general}
        <InsetRow
          icon={Gauge}
          iconColor="var(--s-cpu)"
          label="Poll Interval"
          sublabel="How often the hardware sensors are read"
          for="poll-interval"
        >
          <div class="slide">
            <Slider
              id="poll-interval"
              label="Poll Interval"
              hideLabel
              min={100}
              max={5000}
              step={100}
              bind:value={general.poll_interval_ms}
              format={(v) => `${v}ms`}
            />
          </div>
        </InsetRow>
        <InsetRow icon={ScrollText} iconColor="var(--s-other)" label="Log Level" sublabel="Detail written to the log file">
          <Select label="Log Level" size="sm" options={LOG_LEVELS} bind:value={general.log_level} />
        </InsetRow>
        <InsetRow icon={Cpu} iconColor="var(--s-water)" label="LibreHardwareMonitor" sublabel="Executable used for CPU sensors; edit config.toml to change">
          <span class="path mono" class:unset={!general.lhm_exe_path}>{general.lhm_exe_path ?? 'Not set'}</span>
        </InsetRow>
        <div class="save">
          {#if configStore.dirty}<span class="dirty t-callout"><i aria-hidden="true"></i>Unsaved changes</span>{/if}
          <Button variant="primary" onclick={() => saveCurrentConfig()} loading={configStore.saving}>
            {configStore.saving ? 'Saving...' : 'Save Settings'}
          </Button>
        </div>
      {:else}
        <div class="skel" aria-hidden="true">
          <Skeleton height="16px" width="40%" />
          <Skeleton height="16px" width="25%" />
          <Skeleton height="16px" width="55%" />
        </div>
      {/if}
    </InsetGroup>

    <UpdatesGroup />

    <InsetGroup title="About" id="about">
      <div class="about">
        <AppMark size={64} />
        <div class="abt">
          <p class="t-title3">{appVersion ? `Corsair Hub v${appVersion}` : 'Corsair Hub'}</p>
          <p class="t-callout desc">Lightweight replacement for iCUE — smart fan control for custom water cooling</p>
          <p class="t-callout lic">Apache 2.0 License</p>
        </div>
      </div>
    </InsetGroup>
  </div>
</LargeTitlePage>

<style>
  .banner {
    margin-bottom: var(--sp-4);
  }
  .groups {
    display: flex;
    flex-direction: column;
    gap: var(--sp-8);
  }
  .slide {
    width: 280px;
  }
  .path {
    max-width: 280px;
    font-size: 12px;
    color: var(--t1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .path.unset {
    color: var(--t3);
  }
  .save {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--sp-4);
    min-height: 52px;
    padding: 8px 14px;
  }
  .dirty {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    color: var(--t2);
  }
  .dirty i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--warn);
  }
  .skel {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    padding: var(--sp-4);
  }
  .about {
    display: flex;
    align-items: center;
    gap: var(--sp-5);
    padding: var(--sp-5);
  }
  .abt {
    min-width: 0;
  }
  .desc {
    color: var(--t2);
    margin-top: 4px;
    max-width: 48ch;
  }
  .lic {
    color: var(--t3);
    margin-top: 2px;
  }
</style>
