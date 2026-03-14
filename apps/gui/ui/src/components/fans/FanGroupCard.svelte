<script lang="ts">
  import type { FanGroupConfig, FanMode, CurvePoint } from '../../lib/types';
  import ModeSelector from './ModeSelector.svelte';
  import CurveEditor from './CurveEditor.svelte';
  import PidTuner from './PidTuner.svelte';

  interface Props {
    group: FanGroupConfig;
    currentTemp?: number;
    onchange: (group: FanGroupConfig) => void;
  }

  let { group, currentTemp, onchange }: Props = $props();

  const modeType = $derived(group.mode.type);

  function changeMode(newMode: 'fixed' | 'curve' | 'pid') {
    let mode: FanMode;
    switch (newMode) {
      case 'fixed':
        mode = { type: 'fixed', duty_percent: 50 };
        break;
      case 'curve':
        mode = {
          type: 'curve',
          points: [
            { temp: 30, duty: 25 },
            { temp: 50, duty: 40 },
            { temp: 70, duty: 70 },
            { temp: 85, duty: 100 },
          ],
          hysteresis: 3,
          ramp_rate: 5,
          temp_source: { sensors: ['cpu'], weights: [1] },
        };
        break;
      case 'pid':
        mode = {
          type: 'pid',
          target_temp: 65,
          kp: 2.5,
          ki: 0.15,
          kd: 1.0,
          min_duty: 25,
          max_duty: 100,
          temp_source: { sensors: ['cpu'], weights: [1] },
        };
        break;
    }
    onchange({ ...group, mode });
  }
</script>

<div class="card fan-group-card">
  <div class="header">
    <h4 class="group-name">{group.name}</h4>
    <span class="channels">Ch {group.channels.join(', ')}</span>
  </div>

  <ModeSelector value={modeType} onchange={changeMode} />

  <div class="mode-content">
    {#if group.mode.type === 'fixed'}
      <div class="fixed-control">
        <span class="field-label">Duty</span>
        <div class="slider-row">
          <input
            type="range" min="0" max="100" step="1"
            value={group.mode.duty_percent}
            oninput={(e) => onchange({ ...group, mode: { type: 'fixed', duty_percent: Number(e.currentTarget.value) } })}
          />
          <span class="tabular-nums">{group.mode.duty_percent}%</span>
        </div>
      </div>
    {:else if group.mode.type === 'curve'}
      <CurveEditor
        points={group.mode.points}
        {currentTemp}
        onchange={(points) => onchange({ ...group, mode: { ...group.mode, points } as FanMode & { type: 'curve' } })}
      />
    {:else if group.mode.type === 'pid'}
      <PidTuner
        targetTemp={group.mode.target_temp}
        kp={group.mode.kp}
        ki={group.mode.ki}
        kd={group.mode.kd}
        minDuty={group.mode.min_duty}
        maxDuty={group.mode.max_duty}
        onchange={(vals) => onchange({
          ...group,
          mode: {
            ...group.mode,
            target_temp: vals.targetTemp,
            kp: vals.kp,
            ki: vals.ki,
            kd: vals.kd,
            min_duty: vals.minDuty,
            max_duty: vals.maxDuty,
          } as FanMode & { type: 'pid' }
        })}
      />
    {/if}
  </div>
</div>

<style>
  .fan-group-card {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  .group-name {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
  }
  .channels {
    font-size: 11px;
    color: var(--text-muted);
    font-family: var(--font-mono);
  }
  .mode-content {
    min-height: 40px;
  }
  .fixed-control .field-label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted);
  }
  .slider-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 4px;
  }
  .slider-row span {
    width: 40px;
    text-align: right;
    font-size: 13px;
    font-weight: 500;
  }
</style>
