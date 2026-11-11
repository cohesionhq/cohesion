export default function solution() {
  const old = B.ws().createDocumentFolder('Old drafts');
  for (const d of B.docFolder('Drafts').documents()) if (d.title.startsWith('Untitled')) d.moveTo(old);
}
