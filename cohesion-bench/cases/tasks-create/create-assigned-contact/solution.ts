export default function solution() {
  const priya = B.contact('Priya', 'Shah');
  B.ws().createTask({ title: 'Return the casserole dish to Priya', assignee: { kind: 'contact', contactId: priya.id } });
}
