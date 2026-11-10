export default function solution() {
  const ana = B.ws().createContact({ firstName: 'Ana', lastName: 'Silva' });
  const launch = new Date(B.date(0).getFullYear(), 11, 1, 12);
  if (launch < B.date(0)) launch.setFullYear(launch.getFullYear() + 1);
  launch.setDate(launch.getDate() - 1);
  B.ws().createTask({ title: 'Chase Ana about the launch checklist', assignee: { kind: 'contact', contactId: ana.id }, dueDate: B.iso(launch) });
}
