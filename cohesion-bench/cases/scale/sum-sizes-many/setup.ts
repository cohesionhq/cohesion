export default function setup() {
  const folder = B.ws().createTaskFolder({ title: 'Imported' });
  for (let i = 1; i <= 180; i++) {
    const task = folder.create({ title: `Job ${i}`, size: (((i * 7) % 5) + 1) as TaskSize });
    if (i % 4 === 0) task.complete();
  }
}
