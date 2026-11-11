export default function solution() {
  const doc = B.doc('Cabin');
  doc.contents = doc.contents.replace('pinecone-42', 'cedar-7');
}
