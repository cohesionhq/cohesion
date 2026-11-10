export default function setup() {
  const folder = B.ws().createTaskFolder({ title: 'Imported' });
  for (let i = 1; i <= 200; i++) {
    folder.create({ title: i % 5 === 0 ? `Kitchen: item ${i}` : `Garden: item ${i} (not the kitchen)` });
  }
}
