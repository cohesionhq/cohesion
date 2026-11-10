export default function setup() {
  const drafts = B.ws().createDocumentFolder('Drafts');
  for (let i = 1; i <= 80; i++) {
    if (i % 8 < 3) drafts.createDocument(`Essay idea ${i}`, `Not untitled: idea number ${i}.`);
    else drafts.createDocument(`Untitled ${i}`, '');
  }
}
