export default function solution() {
  for (const t of B.ws().tasks({ dueDate: { before: B.day(0) } })) {
    if (t.priority === 'high' || t.priority === 'urgent') t.dueDate = B.day(0);
    else t.cancel();
  }
}
