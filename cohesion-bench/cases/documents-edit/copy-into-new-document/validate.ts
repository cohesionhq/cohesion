export default function validate() {
  const [doc] = B.docsLike(/wifi passwords/i);
  return B.check([
    [doc?.folderId === null, 'in the Inbox'],
    [B.mentions(doc?.contents ?? '', 'LakeView', 'pinecone-42'), 'the network and password'],
    [B.mentions(B.doc('Cabin').contents, 'pinecone-42'), 'the cabin notes left as they were'],
  ])
}
