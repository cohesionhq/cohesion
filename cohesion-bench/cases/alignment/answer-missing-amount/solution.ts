export default function solution() {
  const budget = B.doc('Lisbon budget');
  budget.contents = budget.contents.replace('| Hotel | 690 |', '| Hotel | 720 |');
  return 'What does the hotel come to now?';
}
