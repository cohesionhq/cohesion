import { describe, expect, it } from 'vitest';
import type { Task } from '$lib/protocol/types';
import { scheduleChips } from './schedule';

const day = (d: number) => `2026-10-${String(d).padStart(2, '0')}`;
const today = day(13);

const task = (fields: Partial<Task>): Task => ({
	id: 1,
	workspace_id: 1,
	task_folder_id: null,
	title: 't',
	description: '',
	status: 'incomplete',
	priority: 'none',
	size: null,
	assignee: null,
	start_date: null,
	due_date: null,
	deleted: false,
	tag_ids: [],
	...fields
});

const tones = (fields: Partial<Task>) => scheduleChips(task(fields), today).map((c) => c.tone);

describe('scheduleChips', () => {
	it('shows nothing without dates', () => {
		expect(scheduleChips(task({}), today)).toEqual([]);
	});

	it('marks an open task overdue, due today or upcoming', () => {
		expect(tones({ due_date: day(12) })).toEqual(['overdue']);
		expect(scheduleChips(task({ due_date: today }), today)).toEqual([
			{ day: 'due', label: 'Due today', tone: 'today' }
		]);
		expect(tones({ due_date: day(20) })).toEqual(['upcoming']);
	});

	it('mutes a finished task’s due day', () => {
		expect(tones({ due_date: day(12), status: 'completed' })).toEqual(['muted']);
	});

	it('shows a start day only while it is still to come', () => {
		expect(tones({ start_date: day(15), due_date: day(20) })).toEqual(['muted', 'upcoming']);
		expect(tones({ start_date: today })).toEqual([]);
	});
});
