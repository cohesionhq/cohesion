import { localDay, shortDay } from '$lib/format/dates';
import type { Task } from '$lib/protocol/types';

/** A date shown on a task row. */
export interface ScheduleChip {
	day: 'start' | 'due';
	label: string;
	tone: 'overdue' | 'today' | 'upcoming' | 'muted';
}

/**
 * What a row shows of a task's days: its due day, with an open task's
 * overdue or due-today state, and a start day still to come. Days are
 * "2026-10-12" strings, which compare in date order; `today` is local.
 */
export function scheduleChips(task: Task, today = localDay()): ScheduleChip[] {
	const open = task.status === 'incomplete';
	const chips: ScheduleChip[] = [];
	if (open && task.start_date !== null && task.start_date > today) {
		chips.push({
			day: 'start',
			label: `Starts ${shortDay(task.start_date, today)}`,
			tone: 'muted'
		});
	}
	if (task.due_date !== null) {
		const due = task.due_date;
		const tone = !open ? 'muted' : due < today ? 'overdue' : due === today ? 'today' : 'upcoming';
		const label = due === today ? 'Due today' : `Due ${shortDay(due, today)}`;
		chips.push({ day: 'due', label, tone });
	}
	return chips;
}
