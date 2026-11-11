export default function validate() {
  return B.answered([[B.mentions(reply, 'deploy'), `Document the deploy process (said: ${reply.slice(0, 200)})`]]);
}
