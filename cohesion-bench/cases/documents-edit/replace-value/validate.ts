export default function validate() {
  const text = B.doc('Cabin').contents;
  return B.check([
    [B.mentions(text, 'cedar-7'), 'the new password'],
    [!B.mentions(text, 'pinecone-42'), 'the old password gone'],
    [B.mentions(text, 'blue pot', 'Thursday', 'LakeView'), 'the rest kept'],
  ])
}
