<script lang="ts">
  interface Props {
    targetTemp: number;
    kp: number;
    ki: number;
    kd: number;
    minDuty: number;
    maxDuty: number;
    onchange: (values: { targetTemp: number; kp: number; ki: number; kd: number; minDuty: number; maxDuty: number }) => void;
  }

  let { targetTemp, kp, ki, kd, minDuty, maxDuty, onchange }: Props = $props();

  function emit() {
    onchange({ targetTemp, kp, ki, kd, minDuty, maxDuty });
  }
</script>

<div class="pid-tuner">
  <div class="field">
    <span class="field-label">Target Temp</span>
    <div class="input-row">
      <input type="range" min="30" max="85" step="1" bind:value={targetTemp} oninput={emit} />
      <span class="tabular-nums">{targetTemp}°C</span>
    </div>
  </div>

  <div class="gains">
    <div class="field">
      <span class="field-label">Kp</span>
      <input type="number" min="0.1" max="20" step="0.1" bind:value={kp} oninput={emit} />
    </div>
    <div class="field">
      <span class="field-label">Ki</span>
      <input type="number" min="0" max="5" step="0.01" bind:value={ki} oninput={emit} />
    </div>
    <div class="field">
      <span class="field-label">Kd</span>
      <input type="number" min="0" max="10" step="0.1" bind:value={kd} oninput={emit} />
    </div>
  </div>

  <div class="range-row">
    <div class="field">
      <span class="field-label">Min Duty</span>
      <div class="input-row">
        <input type="range" min="0" max="100" step="1" bind:value={minDuty} oninput={emit} />
        <span class="tabular-nums">{minDuty}%</span>
      </div>
    </div>
    <div class="field">
      <span class="field-label">Max Duty</span>
      <div class="input-row">
        <input type="range" min="0" max="100" step="1" bind:value={maxDuty} oninput={emit} />
        <span class="tabular-nums">{maxDuty}%</span>
      </div>
    </div>
  </div>
</div>

<style>
  .pid-tuner {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .field-label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    color: var(--text-muted);
  }
  .input-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .input-row span {
    width: 48px;
    text-align: right;
    font-size: 12px;
  }
  .gains {
    display: grid;
    grid-template-columns: 1fr 1fr 1fr;
    gap: 8px;
  }
  .gains input {
    width: 100%;
  }
  .range-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
</style>
