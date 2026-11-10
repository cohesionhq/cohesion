export default function solution() {
  B.ws().createTask({ title: 'Read The Overstory' });
  B.ws().createTask({ title: 'Read Klara and the Sun' });
  B.doc('Reading list').append('- Babel\n');
}
