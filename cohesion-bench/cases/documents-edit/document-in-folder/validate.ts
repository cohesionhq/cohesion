export default function validate() {
  const [doc] = B.docsLike(/book club/i);
  const items = doc ? doc.contents.split('\n').filter((line) => /^\s*([-*+]|\d+[.)])\s/.test(line)) : [];
  return B.check([
    [doc?.folderId === B.docFolder('Reference').id, 'in Reference'],
    [items.length >= 3, `three novels listed (found ${items.length})`],
  ])
}
