export default function setup() {
  const folder = B.ws().createTaskFolder({ title: 'Imported' });
  for (let i = 1; i <= 240; i++) {
    folder.create({ title: i % 4 === 0 ? `TEMP placeholder ${i}` : `Keep: real item ${i} (not TEMP)` });
  }
}
