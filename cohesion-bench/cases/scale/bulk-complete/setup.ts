export default function setup() {
  B.ws().createTag('import');
  for (let i = 1; i <= 200; i++) {
    const task = B.ws().createTask({ title: `Imported item ${i}` });
    if (i <= 150) task.tag('import');
  }
}
