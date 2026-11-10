export default function validate() {
  const [doc] = B.docsLike(/week so far/i);
  const text = doc?.contents ?? '';
  return B.check([
    [doc !== undefined, 'a Week so far document'],
    [B.mentions(text, '5k') || B.mentions(text, 'ran'), 'the run'],
    [B.mentions(text, 'Piranesi'), 'the reading'],
    [B.mentions(text, 'shelf'), 'the shelf'],
  ])
}
