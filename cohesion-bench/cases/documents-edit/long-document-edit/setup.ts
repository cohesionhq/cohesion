export default function setup() {
  const lines = Array.from({ length: 600 }, (_, i) => i === 436 ? 'Section 437: The office closes at 6pm on Fridays.' : `Section ${i + 1}: Policy text number ${i + 1}, unchanged since 2019.`);
  B.ws().createDocument('Handbook', lines.join('\n'));
}
