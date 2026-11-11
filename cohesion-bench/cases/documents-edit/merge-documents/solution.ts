export default function solution() {
  B.ws().createDocument('Meeting digest', B.doc('Kickoff meeting notes').contents + '\n\n' + B.doc('Garden committee').contents);
}
