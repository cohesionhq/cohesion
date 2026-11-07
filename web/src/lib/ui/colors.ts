// The one place an agent colour becomes a class. Agents pick one; a tile
// standing in for a missing icon (an extension, a connected service) is
// given one hashed from a stable string, so it never changes.

import type { IntelligenceAgentColor } from '$lib/protocol/types';

/** The `bg-agent-*` fill for each agent colour. */
export const agentFill: Record<IntelligenceAgentColor, string> = {
	blue: 'bg-agent-blue',
	green: 'bg-agent-green',
	orange: 'bg-agent-orange',
	purple: 'bg-agent-purple',
	red: 'bg-agent-red',
	pink: 'bg-agent-pink',
	teal: 'bg-agent-teal',
	yellow: 'bg-agent-yellow'
};

const TILE_COLORS = Object.values(agentFill);

/** A `bg-agent-*` class hashed from `seed`. */
export function tileColor(seed: string): string {
	let hash = 0;
	for (const c of seed) hash = (hash * 31 + c.charCodeAt(0)) >>> 0;
	return TILE_COLORS[hash % TILE_COLORS.length];
}
