import { describe, expect, it } from 'vitest';
import { fileSize } from './bytes';

describe('fileSize', () => {
	it('counts small sizes in bytes and larger ones in decimal units', () => {
		expect(fileSize(1)).toBe('1 byte');
		expect(fileSize(999)).toBe('999 bytes');
		expect(fileSize(1_310_720)).toBe('1.31 MB');
		expect(fileSize(48_234_496)).toBe('48.2 MB');
		expect(fileSize(2_000_000_000)).toBe('2 GB');
	});
});
