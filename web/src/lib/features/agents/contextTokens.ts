/**
 * An agent's context size in thousands ("18K"); under a thousand it stays exact. While the agent
 * has no live context to measure, an em dash in the shape of the thousands it will become ("—K").
 */
export function contextTokens(tokens: number | null): string {
	if (tokens === null) return '—K';
	return tokens < 1000 ? String(tokens) : `${Math.round(tokens / 1000)}K`;
}

/** The count as a screen reader says it, where "—K" would be read as "dash k". */
export function contextTokensLabel(tokens: number | null): string {
	return tokens === null ? 'Unknown' : contextTokens(tokens);
}
