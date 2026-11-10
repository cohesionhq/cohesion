export default function validate() {
  const [doc] = B.docsLike(/this week/i, B.ANY);
  const day = /monday|tuesday|wednesday|thursday|friday|saturday|sunday|\d{4}-\d{2}-\d{2}|\b\d{1,2}(st|nd|rd|th)?\s+(jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)|today|tomorrow/i;
  const sections = (doc?.contents ?? '').split(/^(?=#+\s)/m).filter((s) => /^#+\s/.test(s) && day.test(s.split('\n')[0]));
  const soon = B.tasks().filter((t) => t.status === 'incomplete' && t.dueDate !== null && t.dueDate >= B.day(0) && t.dueDate <= B.day(6));
  const misplaced = soon.filter((t) => sections.findIndex((s) => B.mentions(s, t.title)) !== [0, 1, 2, 3, 4, 5, 6].find((n) => t.dueDate === B.day(n)));
  return B.check([
    [doc !== undefined, 'a This week document'],
    [sections.length === 7, `seven day headings (found ${sections.length})`],
    [misplaced.length === 0, `each task under its day (misplaced: ${misplaced.map((t) => t.title).join(', ') || 'none'})`],
    [!sections.some((s) => B.mentions(s, 'library books')), 'not the overdue library books'],
  ]);
}
