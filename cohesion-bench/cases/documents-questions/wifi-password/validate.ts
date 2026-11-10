export default function validate() {
  return B.answered([
    [B.mentions(reply, 'pinecone-42'), 'pinecone-42'],
  ])
}
