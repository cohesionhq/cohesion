export default function validate() {
  return B.check([
    [B.findDocFolder('Old stuff') === null, 'Old stuff gone'],
    [B.doc('Old router manual').folderId === B.docFolder('Reference').id, 'the router manual in Reference'],
    [B.doc('2019 tax notes').folderId === B.docFolder('Reference').id, 'the tax notes in Reference'],
  ])
}
