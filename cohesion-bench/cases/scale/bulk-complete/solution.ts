export default function solution() {
  for (const t of B.ws().tasks({ tag: 'import' })) t.complete();
}
