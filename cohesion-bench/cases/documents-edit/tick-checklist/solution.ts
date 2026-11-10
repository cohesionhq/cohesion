export default function solution() {
  const doc = B.doc('Packing list');
  doc.contents = doc.contents.replace('- [ ] Sunscreen', '- [x] Sunscreen');
}
