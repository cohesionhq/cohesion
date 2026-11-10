export default function setup() {
  for (const title of ['Collect the parcel from the post office', 'Pay the council tax', 'Book a physio appointment', 'Descale the shower head']) {
    B.ws().createTask({ title });
  }
}
