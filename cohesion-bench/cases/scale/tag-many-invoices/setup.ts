export default function setup() {
  const folder = B.ws().createTaskFolder({ title: 'Imported' });
  for (let i = 1; i <= 250; i++) {
    folder.create({ title: i % 6 === 0 ? `Chase invoice #${2000 + i}` : `Follow up on ticket ${i}` });
  }
}
