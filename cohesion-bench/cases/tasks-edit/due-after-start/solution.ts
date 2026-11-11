export default function solution() {
  for (const t of B.taskFolder('Launch').tasks({})) {
    const [y, m, d] = t.startDate!.split('-').map(Number);
    t.dueDate = B.iso(new Date(y, m - 1, d + 2, 12));
  }
}
