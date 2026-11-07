<!-- App icon tile for the About group; the same mark as the title bar, drawn large. -->
<script lang="ts">
  interface Props {
    size?: number;
  }
  let { size = 64 }: Props = $props();
</script>

<svg class="mark" viewBox="0 0 64 64" width={size} height={size} aria-hidden="true">
  <defs>
    <linearGradient id="about-mark-fill" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#9DB4FF" />
      <stop offset="1" stop-color="#6F7DF2" />
    </linearGradient>
    <linearGradient id="about-mark-sheen" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#fff" stop-opacity="0.38" />
      <stop offset="0.5" stop-color="#fff" stop-opacity="0" />
    </linearGradient>
  </defs>
  <rect x="1" y="1" width="62" height="62" rx="17" fill="url(#about-mark-fill)" />
  <rect x="1" y="1" width="62" height="62" rx="17" fill="url(#about-mark-sheen)" />
  <rect x="1.5" y="1.5" width="61" height="61" rx="16.5" fill="none" stroke="#fff" stroke-opacity="0.28" />
  <path
    d="M14 37h8.4l5.2-13.6 8.4 21.6 5.2-8h8.8"
    fill="none"
    stroke="#0B1230"
    stroke-width="5"
    stroke-linecap="round"
    stroke-linejoin="round"
  />
</svg>

<style>
  .mark {
    flex: none;
    display: block;
    filter: drop-shadow(0 2px 3px rgb(0 0 0 / 0.4)) drop-shadow(0 10px 18px rgb(0 0 0 / 0.35));
  }
</style>
