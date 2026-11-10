export default function setup() {
  const notes = B.ws().createDocumentFolder('House notes');
  for (let i = 1; i <= 120; i++) {
    notes.createDocument(`House note ${i}`, `Note ${i}. The ${['fridge', 'oven', 'washer', 'dryer'][i % 4]} was checked in week ${i}. Nothing to report.`);
  }
  notes.createDocument('House note 121', 'The boiler engineer came. The boiler service code is 7741. Next visit in a year.');
}
