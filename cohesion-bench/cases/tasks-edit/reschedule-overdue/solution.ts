export default function solution() {
  for (const t of B.ws().tasks({ dueDate: { before: B.day(0) } })) t.dueDate = B.day(3);
}
