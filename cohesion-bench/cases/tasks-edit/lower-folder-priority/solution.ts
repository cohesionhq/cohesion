export default function solution() {
  for (const t of B.taskFolder('Work').tasks({})) t.priority = 'low';
}
