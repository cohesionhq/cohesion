export default function solution() {
  const seen = new Set<string>();
  for (const t of B.taskFolder('Imported').tasks({}).sort((a, b) => a.id - b.id)) {
    if (seen.has(t.title)) t.cancel();
    seen.add(t.title);
  }
}
