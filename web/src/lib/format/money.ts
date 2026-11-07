// Dollar amounts: catalog prices (per 1M tokens) and what model calls cost.

/** "$3.00"; amounts under a cent keep two significant digits ("$0.0042"). */
export function usd(amount: number): string {
	if (amount > 0 && amount < 0.01) return `$${Number(amount.toPrecision(2))}`;
	return `$${amount.toFixed(2)}`;
}

/** A catalog price per 1M tokens, or "—" when the catalog has none. */
export function perMillion(cost: number | null | undefined): string {
	if (cost === null || cost === undefined) return '—';
	return usd(cost);
}
