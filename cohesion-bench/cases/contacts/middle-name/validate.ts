export default function validate() {
  return B.check([[B.contact('Helen', 'Carter').middleName === 'Anne', 'middle name Anne']])
}
