export default function solution() {
  const done = B.ws().createTaskFolder({ title: 'Done' });
  for (const t of B.ws().tasks({ status: 'completed' })) t.moveTo(done);
}
