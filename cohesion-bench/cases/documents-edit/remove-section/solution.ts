export default function solution() {
  const doc = B.doc('Garden plan');
  doc.contents = doc.contents.replace('## Old ideas\n\nA pond by the fence. A second shed.\n\n', '');
}
