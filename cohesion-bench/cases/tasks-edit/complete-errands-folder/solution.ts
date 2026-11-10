export default function solution() {
  for (const t of B.taskFolder('Errands').tasks({})) t.complete();
}
