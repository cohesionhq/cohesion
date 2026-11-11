export default function validate() {
  return B.check([[B.ws().emoji === '🏡', `emoji 🏡 (was ${B.ws().emoji})`]])
}
