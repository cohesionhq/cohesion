export default function validate() {
  const [digest] = B.docsLike(/meeting digest/i);
  const text = digest?.contents ?? '';
  return B.check([
    [digest !== undefined, 'a Meeting digest'],
    [B.mentions(text, '1 December'), 'the kickoff decisions'],
    [B.mentions(text, 'wildflower'), 'the garden committee decisions'],
    [B.docsLike(/kickoff meeting notes|garden committee/i).length === 2, 'the originals kept'],
  ])
}
