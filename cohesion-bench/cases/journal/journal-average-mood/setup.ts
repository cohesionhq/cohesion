export default function setup() {
  const moods = [7, 5, 8, 6, 4, 9, 7, 6, 5, 8, 7, 6, 7, 6];
  moods.forEach((mood, i) => {
    B.ws().journal(B.day(-1 - i)).contents = `Mood: ${mood}/10. Day ${i + 1} of the fortnight; nothing much else.`;
  });
}
