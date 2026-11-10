export default function validate() {
  const tokyo = B.findDocFolder('Tokyo 2027');
  const ideas = B.docsLike(/^ideas$/i);
  return B.check([
    [tokyo?.parentId === B.docFolder('Trips').id, 'Tokyo 2027 inside Trips'],
    [ideas.length === 1 && ideas[0].folderId === tokyo?.id, 'Ideas in Tokyo 2027'],
  ])
}
