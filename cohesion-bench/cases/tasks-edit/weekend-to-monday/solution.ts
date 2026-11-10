export default function solution() {
  for (const t of B.ws().tasks({ status: 'incomplete' })) {
    if (t.dueDate === null) continue;
    const [y, m, d] = t.dueDate.split('-').map(Number);
    const at = new Date(y, m - 1, d, 12);
    const shift = at.getDay() === 6 ? 2 : at.getDay() === 0 ? 1 : 0;
    if (shift) { at.setDate(at.getDate() + shift); t.dueDate = B.iso(at); }
  }
}
