export default function solution() {
  for (const t of B.ws().tasks({ tag: 'errand' })) t.complete();
}
