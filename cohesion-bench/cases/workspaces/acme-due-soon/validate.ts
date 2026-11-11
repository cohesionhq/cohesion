export default function validate() {
  return B.answered([
    [B.mentions(reply, 'login'), 'the login bug'],
    [B.mentions(reply, 'pull request'), "Leo's pull request"],
    [B.mentions(reply, 'release notes'), 'the release notes'],
    [!B.mentions(reply, 'dentist') && !B.mentions(reply, 'tap'), 'nothing from Personal'],
  ]);
}
