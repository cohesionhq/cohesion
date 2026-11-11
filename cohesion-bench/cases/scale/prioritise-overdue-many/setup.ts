export default function setup() {
  const folder = B.ws().createTaskFolder({ title: 'Imported' });
  for (let i = 1; i <= 160; i++) {
    const task = folder.create({ title: `Item ${i}`, dueDate: B.day((i % 21) - 10) });
    if (i % 9 === 0) task.complete();
  }
}
