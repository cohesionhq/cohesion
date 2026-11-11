export default function validate() {
  const [doc] = B.newDocs('Acme');
  const text = doc?.contents ?? '';
  return B.check([
    [/^#+\s*Build/im.test(text) && /^#+\s*Release/im.test(text) && /^#+\s*Rollback/im.test(text), 'a document with the three headings'],
    [B.task('Document the deploy process', 'Acme').priority === 'high', 'the backlog task high priority'],
  ])
}
