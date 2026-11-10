export default function validate() {
  const made = B.newDocs(B.ANY);
  return B.acted([[made.length >= 1 && made[0].contents.trim().length > 80, `a document with some ideas (made ${made.length})`]]);
}
