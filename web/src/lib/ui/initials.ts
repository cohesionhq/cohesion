/** A name's initials, at most two: its first word's and its last word's first letters, capitalized; "" for no name. */
export function initials(name: string | null): string {
	const words = (name ?? '').trim().split(/\s+/).filter(Boolean);
	if (words.length === 0) return '';
	const first = [...words[0]][0];
	const last = words.length > 1 ? [...words[words.length - 1]][0] : '';
	return (first + last).toUpperCase();
}
