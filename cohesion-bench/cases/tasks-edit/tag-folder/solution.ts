export default function solution() {
  for (const t of B.taskFolder('Finance').tasks({})) t.tag('money');
}
