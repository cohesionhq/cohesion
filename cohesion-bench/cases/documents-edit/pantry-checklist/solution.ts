export default function solution() {
  const pantry = B.doc('Pantry staples');
  pantry.contents = pantry.contents.replace(/^- /gm, '- [ ] ');
}
