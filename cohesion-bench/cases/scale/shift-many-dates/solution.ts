export default function solution() {
  for (const t of B.tasks()) {
    if (/^Weekly report/.test(t.title) && t.dueDate && t.dueDate < B.day(0)) {
      const [y, m, d] = t.dueDate.split('-').map(Number);
      t.dueDate = B.iso(new Date(y, m - 1, d + 7, 12));
    }
  }
}
