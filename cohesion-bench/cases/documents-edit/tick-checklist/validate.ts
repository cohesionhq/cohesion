export default function validate() {
  const text = B.doc('Packing list').contents;
  return B.check([
    [/^- \[x\] Sunscreen/im.test(text), 'sunscreen ticked'],
    [/^- \[ \] Passport/m.test(text) && /^- \[ \] Swimsuit/m.test(text), 'the others left unticked'],
    [/^- \[x\] Adapter plug/m.test(text), 'the adapter plug still ticked'],
  ])
}
