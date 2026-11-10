export default function validate() {
  const docs = B.docsLike(/camping checklist/i);
  const items = docs.length ? docs[0].contents.split('\n').filter((line) => /^\s*[-*+]\s|^\s*\d+[.)]\s/.test(line)) : [];
  return B.check([
    [docs.length === 1, `one Camping checklist (found ${docs.length})`],
    [items.length >= 6, `at least six items (found ${items.length})`],
  ])
}
