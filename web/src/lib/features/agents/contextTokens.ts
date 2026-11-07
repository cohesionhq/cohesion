/**
 * An agent's context size in thousands ("18k"); under a thousand it stays exact. While the agent
 * has no live context to measure, an em dash in the shape of the thousands it will become ("—k").
 */
export function contextTokens(tokens: number | null): string {
	if (tokens === null) return '—k';
	return tokens < 1000 ? String(tokens) : `${Math.round(tokens / 1000)}k`;
}

/** The count as a screen reader says it, where "—k" would be read as "dash k". */
export function contextTokensLabel(tokens: number | null): string {
	return tokens === null ? 'Unknown' : contextTokens(tokens);
}
