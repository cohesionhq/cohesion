const docs = cohesion.workspace('Personal').documents().filter((d) => /packing list/i.test(d.title));
const items = docs.length ? docs[0].contents.split('\n').filter((line) => /^\s*[-*+]\s|^\s*\d+[.)]\s/.test(line)) : [];
({
  pass: docs.length === 1 && items.length >= 5,
  reason: docs.length === 1 ? `${items.length} list items` : `${docs.length} documents called Packing list`,
})
