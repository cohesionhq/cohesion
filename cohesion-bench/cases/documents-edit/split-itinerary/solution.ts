export default function solution() {
  const folder = B.docFolder('Lisbon 2026');
  const days = B.doc('Lisbon itinerary').contents.split(/^## /m).slice(1);
  days.forEach((section, i) => folder.createDocument(`Lisbon day ${i + 1}`, `# ${section.trim()}\n`));
}
