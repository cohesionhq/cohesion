/**
 * What a working line says: what the daemon says the agent is doing, trailing
 * off, or "Working…" with nothing to go on (an actor session, a message still on
 * its way). A script's title is the model's own words, so any ending of its own
 * gives way to the ellipsis.
 */
export function workingCaption(activity?: string | null): string {
	return `${activity?.replace(/[\s.…]+$/, '') || 'Working'}…`;
}
