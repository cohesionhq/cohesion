export default function solution() {
  const doc = B.doc('Onboarding guide', 'Acme');
  doc.contents = doc.contents.replaceAll('recieve', 'receive');
}
