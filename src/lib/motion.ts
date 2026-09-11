import { cubicOut } from 'svelte/easing';
import { prefersReducedMotion } from 'svelte/motion';

const ms = (duration: number) => (prefersReducedMotion.current ? 0 : duration);

/** Shared enter/exit presets so every surface moves the same way and honors reduced motion. */
export const motion = {
  popover: () => ({ y: -6, duration: ms(160), easing: cubicOut }),
  drawer: () => ({ x: 24, duration: ms(220), easing: cubicOut }),
  toast: () => ({ y: 12, duration: ms(220), easing: cubicOut }),
  fade: () => ({ duration: ms(140) }),
  slide: () => ({ axis: 'x' as const, duration: ms(240), easing: cubicOut }),
};
