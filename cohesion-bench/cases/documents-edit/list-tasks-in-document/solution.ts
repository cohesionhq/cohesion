export default function solution() {
  B.ws().createDocument('Errands list', B.ws().tasks({ tag: 'errand' }).map((t) => `- ${t.title}`).join('\n'));
}
