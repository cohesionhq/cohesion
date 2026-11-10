export default function solution() {
  B.docFolder('Trips').createFolder('Rome 2027').createDocument('Rome itinerary', '## Day 1\nColosseum\n\n## Day 2\nVatican\n\n## Day 3\nTrastevere\n');
  B.ws().createTask({ title: 'Book flights to Rome', dueDate: B.day(30) });
  B.ws().createTask({ title: 'Book the Rome hotel', dueDate: B.day(30) });
}
