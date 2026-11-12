// Joins the one-item fetches that many components make at once (a row each
// asking for its own grants, a card each for its own link) into one request
// for every item, so a screen of N items costs one round trip, not N.

/**
 * A fetch of one key's value that waits a microtask, so every key asked for
 * in the same task goes to `fetchMany` together, once each. `fetchMany`
 * answers with a value for each key it knows; `missing(key)` stands in for
 * one it leaves out.
 */
export function batched<K, V>(
	fetchMany: (keys: K[]) => Promise<Map<K, V>>,
	missing: (key: K) => V
): (key: K) => Promise<V> {
	let waiting: Map<K, { resolve: (value: V) => void; reject: (error: unknown) => void }[]> | null =
		null;
	const flush = async () => {
		const asked = waiting!;
		waiting = null;
		try {
			const found = await fetchMany([...asked.keys()]);
			for (const [key, callers] of asked)
				for (const { resolve } of callers) resolve(found.has(key) ? found.get(key)! : missing(key));
		} catch (error) {
			for (const callers of asked.values()) for (const { reject } of callers) reject(error);
		}
	};
	return (key) =>
		new Promise<V>((resolve, reject) => {
			if (!waiting) {
				waiting = new Map();
				queueMicrotask(() => void flush());
			}
			const callers = waiting.get(key) ?? [];
			callers.push({ resolve, reject });
			waiting.set(key, callers);
		});
}

/** `rows` grouped by `key`, for a [`batched`] fetch whose reply lists every key's rows together. */
export function groupBy<K, R>(rows: R[], key: (row: R) => K): Map<K, R[]> {
	const groups = new Map<K, R[]>();
	for (const row of rows) {
		const group = groups.get(key(row));
		if (group) group.push(row);
		else groups.set(key(row), [row]);
	}
	return groups;
}
