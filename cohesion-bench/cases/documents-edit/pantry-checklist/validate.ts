export default function validate() {
  const text = B.doc('Pantry staples').contents;
  const items = ['Rice', 'Pasta', 'Tinned tomatoes', 'Red lentils', 'Coffee beans'];
  return B.check([
    [items.every((item) => new RegExp(`^\\s*[-*+]\\s+\\[ \\]\\s+${item}\\s*$`, 'im').test(text)), 'each item an unticked checkbox'],
    [!/\[x\]/i.test(text), 'nothing ticked'],
  ]);
}
