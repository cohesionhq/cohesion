export default function solution() {
  for (const t of B.taskFolder('Health').tasks({})) if (t.priority === 'none') t.priority = 'medium';
}
