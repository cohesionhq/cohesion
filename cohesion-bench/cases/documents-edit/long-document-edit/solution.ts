export default function solution() {
  const doc = B.doc('Handbook');
  doc.contents = doc.contents.replace('closes at 6pm on Fridays', 'closes at 5pm on Fridays');
}
