import { describe, expect, it } from 'vitest';
import { storageRows, storageShare } from './storageAreas';

describe('storage breakdown', () => {
	it('lists the areas as sent, then free space, adding up to the file', () => {
		const size = {
			bytes: 1000,
			free_bytes: 100,
			areas: [
				{ area: 'agentChats' as const, bytes: 700 },
				{ area: 'other' as const, bytes: 200 }
			]
		};
		const rows = storageRows(size);
		expect(rows.map((row) => row.label)).toEqual(['Agent chats', 'Everything else', 'Free space']);
		expect(rows.reduce((sum, row) => sum + row.bytes, 0)).toBe(size.bytes);
	});

	it('rounds a share, and shows a sliver as under 1%', () => {
		expect(storageShare(444, 1000)).toBe('44%');
		expect(storageShare(3, 1000)).toBe('<1%');
		expect(storageShare(0, 1000)).toBe('0%');
		expect(storageShare(0, 0)).toBe('0%');
	});
});
