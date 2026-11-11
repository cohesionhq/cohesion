export default function solution() {
  for (const d of B.ws('Acme').documents()) {
    if (/Falcon/.test(d.contents)) d.contents = d.contents.replace(/Falcon/g, 'Kestrel');
  }
}
