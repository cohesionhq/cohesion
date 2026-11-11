export default function solution() {
  for (const item of ['flights', 'hotel', 'food']) B.ws().createTask({ title: `Pay for the Lisbon ${item}`, dueDate: B.day(10) });
}
