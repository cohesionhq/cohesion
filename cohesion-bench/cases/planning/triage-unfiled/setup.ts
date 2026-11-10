export default function setup() {
  for (const title of ['Buy bin bags', 'Pay the council tax', 'Book a physio appointment', 'Descale the shower head']) {
    B.ws().createTask({ title });
  }
}
