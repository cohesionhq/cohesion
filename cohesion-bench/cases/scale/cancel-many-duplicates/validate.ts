export default function validate() {
  const tasks = B.taskFolder('Imported').tasks({ status: B.ALL }).sort((a, b) => a.id - b.id);
  const seen = new Set<string>();
  const wrong = tasks.filter((t) => {
    const copy = seen.has(t.title);
    seen.add(t.title);
    return (t.status === 'cancelled') !== copy;
  });
  return B.check([[wrong.length === 0, `each newer copy cancelled and nothing else (${wrong.length} wrong)`]]);
}
