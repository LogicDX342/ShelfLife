import type { FileDecayState } from '$lib/types';

export function decayClass(state: FileDecayState) {
  return `state-${state.toLowerCase()}`;
}
