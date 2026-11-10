export default function setup() {
  const folder = B.ws().createTaskFolder({ title: 'Imported' });
  for (let i = 1; i <= 150; i++) folder.create({ title: `Order part ${i}` });
  for (let i = 3; i <= 150; i += 7) folder.create({ title: `Order part ${i}` });
}
