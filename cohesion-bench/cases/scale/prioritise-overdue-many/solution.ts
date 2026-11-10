export default function solution() {
  for (const t of B.taskFolder('Imported').tasks({})) if (t.dueDate !== null && t.dueDate < B.day(0)) t.priority = 'high';
}
