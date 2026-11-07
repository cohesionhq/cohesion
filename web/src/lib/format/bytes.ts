// Sizes on disk: the database file in Settings › Storage.

const UNITS = ['bytes', 'KB', 'MB', 'GB', 'TB'];

/** "48.2 MB": decimal units, three significant digits from a kilobyte up. */
export function fileSize(bytes: number): string {
	let unit = 0;
	let value = bytes;
	while (value >= 1000 && unit < UNITS.length - 1) {
		value /= 1000;
		unit += 1;
	}
	if (unit === 0) return bytes === 1 ? '1 byte' : `${bytes} bytes`;
	return `${Number(value.toPrecision(3))} ${UNITS[unit]}`;
}
