<script lang="ts">
  import type { FanGroupConfig, FanMode } from '../../lib/types';
  import ModeSelector from './ModeSelector.svelte';
  import CurveEditor from './CurveEditor.svelte';
  import PidTuner from './PidTuner.svelte';

  interface Props {
    group: FanGroupConfig;
    currentTemp?: number;
    onchange: (group: FanGroupConfig) => void;
    expanded?: boolean;
  }

  let { group, currentTemp, onchange, expanded = false }: Props = $props();

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

  function updateHysteresis(val: number) {
    if (group.mode.type !== 'curve') return;
    onchange({ ...group, mode: { ...group.mode, hysteresis: val } as FanMode & { type: 'curve' } });
  }

  function updateRampRate(val: number) {
    if (group.mode.type !== 'curve') return;
    onchange({ ...group, mode: { ...group.mode, ramp_rate: val } as FanMode & { type: 'curve' } });
  }
</script>

<div class="fan-group-card" class:expanded>
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
        {expanded}
      />
      <div class="curve-params">
        <div class="param">
          <span class="field-label">Hysteresis</span>
          <div class="slider-row">
            <input
              type="range" min="1" max="10" step="1"
              value={group.mode.hysteresis}
              oninput={(e) => updateHysteresis(Number(e.currentTarget.value))}
            />
            <span class="tabular-nums">{group.mode.hysteresis}°C</span>
          </div>
        </div>
        <div class="param">
          <span class="field-label">Ramp Rate</span>
          <div class="slider-row">
            <input
              type="range" min="1" max="20" step="1"
              value={group.mode.ramp_rate}
              oninput={(e) => updateRampRate(Number(e.currentTarget.value))}
            />
            <span class="tabular-nums">{group.mode.ramp_rate}%/s</span>
          </div>
        </div>
      </div>
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
  .fan-group-card:not(.expanded) {
    background: var(--bg-card);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 16px;
  }
  .header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  .group-name {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
  }
  .expanded .group-name {
    font-size: 20px;
  }
  .channels {
    font-size: 13px;
    color: var(--text-muted);
    font-family: var(--font-mono);
  }
  .mode-content {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
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
    width: 48px;
    text-align: right;
    font-size: 13px;
    font-weight: 500;
  }
  .curve-params {
    display: flex;
    gap: 24px;
    margin-top: 12px;
  }
  .param {
    flex: 1;
  }
  .param .field-label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted);
  }
</style>
