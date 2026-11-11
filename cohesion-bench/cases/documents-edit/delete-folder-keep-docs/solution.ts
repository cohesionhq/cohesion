export default function solution() {
  const old = B.docFolder('Old stuff');
  for (const doc of old.documents()) doc.moveTo(B.docFolder('Reference'));
  old.delete();
}
