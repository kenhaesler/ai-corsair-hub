// Type for a lucide-svelte icon component passed as a prop.
// All lucide icon classes share the same shape, so one alias covers them.
export type IconComponent = typeof import('lucide-svelte/icons/gauge').default;
