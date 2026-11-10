export default function solution() {
  for (const t of B.taskFolder('Imported').tasks({})) if (t.title.startsWith('TEMP')) t.cancel();
}
