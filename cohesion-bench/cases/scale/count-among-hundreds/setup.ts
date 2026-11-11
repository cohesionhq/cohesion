export default function setup() {
  const folder = B.ws().createTaskFolder({ title: 'Imported' });
  for (let i = 1; i <= 300; i++) {
    const title = i % 7 === 0 ? `Send invoice #${1000 + i} to client ${i % 13}` : `Follow up on ticket ${i}`;
    const task = folder.create({ title });
    if (i % 5 === 0) task.complete();
  }
}
