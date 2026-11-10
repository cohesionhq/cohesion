export default function solution() {
  const doc = B.doc('Kickoff meeting notes');
  doc.contents = doc.contents.replace('Attendees: Ana, Bo', 'Attendees: Ana, Bo, Cy');
}
