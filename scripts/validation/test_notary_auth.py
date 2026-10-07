"""Credential-selection tests. All values are synthetic, never real keys."""
import os
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[2]
AUTH = ROOT / 'scripts/macos/notary_auth.sh'
KEYCHAIN = ROOT / 'scripts/macos/ci_keychain.sh'
NAMES = ('APPLE_API_KEY', 'APPLE_API_KEY_ID', 'APPLE_API_ISSUER', 'APPLE_ID',
         'APPLE_APP_SPECIFIC_PASSWORD', 'APPLE_TEAM_ID')
API = dict(zip(NAMES[:3], ('synthetic-key', 'KEY1234567', 'synthetic-issuer')))
ACCOUNT = dict(zip(NAMES[3:], ('test@example.invalid', 'synthetic-password', 'TEAM123456')))


class NotaryAuthTests(unittest.TestCase):
    def run_auth(self, credentials, extra='printf "%s" "$VOLT_NOTARY_AUTH"'):
        env = os.environ.copy()
        for name in NAMES:
            env.pop(name, None)
        env.update(credentials)
        return subprocess.run(['bash', '-c', f'set -eu; source "{AUTH}"; '
                               f'volt_validate_notary_auth; {extra}'], env=env,
                              capture_output=True, text=True)

    def test_accepts_complete_api_credentials(self):
        result = self.run_auth(API)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, 'api')

    def test_accepts_complete_account_credentials(self):
        result = self.run_auth(ACCOUNT)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, 'account')

    def test_rejects_missing_partial_and_mixed_credentials_without_leaks(self):
        cases = [{}] + [{k: v} for k, v in {**API, **ACCOUNT}.items()]
        cases += [{**API, **ACCOUNT}, {**API, 'APPLE_ID': ACCOUNT['APPLE_ID']},
                  {**ACCOUNT, 'APPLE_API_KEY': API['APPLE_API_KEY']}]
        for credentials in cases:
            with self.subTest(names=list(credentials)):
                result = self.run_auth(credentials)
                self.assertNotEqual(result.returncode, 0)
                for value in credentials.values():
                    self.assertNotIn(value, result.stdout + result.stderr)

    def test_profile_uses_disposable_keychain_and_hides_tool_stdout(self):
        extra = '''
SIGNING_KEYCHAIN=/synthetic/disposable.keychain-db
xcrun() {
  [[ "$1" == notarytool && "$2" == store-credentials && "$3" == volt-ci-notary ]]
  [[ "$4" == --keychain && "$5" == "$SIGNING_KEYCHAIN" ]]
  [[ "$6" == --apple-id && "$7" == "$APPLE_ID" ]]
  [[ "$8" == --team-id && "$9" == "$APPLE_TEAM_ID" ]]
  [[ "${10}" == --password && "${11}" == "$APPLE_APP_SPECIFIC_PASSWORD" ]]
  echo synthetic-private-tool-output
}
volt_store_notary_profile
[[ "$NOTARYTOOL_PROFILE" == volt-ci-notary ]]
[[ "$NOTARYTOOL_KEYCHAIN" == "$SIGNING_KEYCHAIN" ]]
'''
        result = self.run_auth(ACCOUNT, extra)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, '')

    def test_signing_identity_accepts_short_name_full_name_and_single_default(self):
        identity = 'Developer ID Application: Test Developer (TEAM123456)'
        for entered in ('', identity, 'Test Developer (TEAM123456)'):
            env = os.environ.copy()
            env.update(TEST_ENTERED=entered, TEST_DETECTED=identity)
            result = subprocess.run(['bash', '-c', f'source "{AUTH}"; '
                                     'volt_resolve_signing_identity "$TEST_ENTERED" "$TEST_DETECTED"'],
                                    env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, identity)

    def test_identity_rejects_ambiguous_default_and_wrong_certificate(self):
        first = 'Developer ID Application: Test Developer (TEAM123456)'
        second = 'Developer ID Application: Other Developer (TEAM654321)'
        cases = [('', first + '\n' + second), ('', ''),
                 ('Test Developer (WRONG12345)', first),
                 ('Apple Development: Test Developer (TEAM123456)', first)]
        for entered, detected in cases:
            env = os.environ.copy()
            env.update(TEST_ENTERED=entered, TEST_DETECTED=detected)
            result = subprocess.run(['bash', '-c', f'source "{AUTH}"; '
                                     'volt_resolve_signing_identity "$TEST_ENTERED" "$TEST_DETECTED"'],
                                    env=env, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, '')

    def test_setup_cannot_run_noninteractively_or_change_settings(self):
        result = subprocess.run(['bash', str(ROOT / 'scripts/macos/setup_github_signing.sh')],
                                input='', capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('interactively', result.stderr)

    def test_shell_syntax(self):
        for script in ('notary_auth.sh', 'ci_keychain.sh', 'ci_sign_app.sh', 'sign_app.sh', 'setup_github_signing.sh'):
            result = subprocess.run(['bash', '-n', str(ROOT / 'scripts/macos' / script)],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_ci_search_list_preserves_spaces_and_restores_original_entries(self):
        script = '''
security() {
  if [[ "$*" == 'list-keychains -d user' ]]; then
    printf '    "/original/login.keychain-db"\n    "/original/path with spaces.keychain-db"\n'
  else
    [[ "$1 $2 $3 $4" == 'list-keychains -d user -s' ]] || return 1
    shift 4
    printf '<%s>\n' "$@"
  fi
}
SIGNING_KEYCHAIN='/temporary/signing keychain-db'
volt_capture_keychain_search_list
volt_add_signing_keychain
printf 'RESTORE\n'
volt_restore_keychain_search_list
'''
        result = subprocess.run(['bash', '-c', f'set -eu; source "{KEYCHAIN}"; {script}'],
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, '</temporary/signing keychain-db>\n' +
                         '</original/login.keychain-db>\n</original/path with spaces.keychain-db>\n'
                         'RESTORE\n</original/login.keychain-db>\n</original/path with spaces.keychain-db>\n')

    def test_ci_identity_requires_exact_valid_developer_id(self):
        identity = 'Developer ID Application: Test Developer (TEAM123456)'
        for detected, valid in ((identity, True), (identity + ' extra', False),
                                ('Apple Development: Test Developer (TEAM123456)', False),
                                ('', False)):
            env = os.environ.copy()
            env.update(SIGNING_IDENTITY=identity, SIGNING_KEYCHAIN='/temporary/test.keychain-db',
                       TEST_DETECTED=detected)
            script = '''
security() {
  [[ "$1" == find-identity ]] || return 1
  if [[ -n "$TEST_DETECTED" ]]; then
    printf '  1) ABCDEF "%s"\n     1 valid identities found\n' "$TEST_DETECTED"
  else
    printf '     0 valid identities found\n'
  fi
}
volt_validate_ci_identity
'''
            result = subprocess.run(['bash', '-c', f'set -eu; source "{KEYCHAIN}"; {script}'],
                                    env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode == 0, valid, result.stderr)

    def test_ci_capture_rejects_unexpected_output(self):
        result = subprocess.run(['bash', '-c', f'set -eu; source "{KEYCHAIN}"; '
                                 'security() { echo unexpected; }; volt_capture_keychain_search_list'],
                                capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)

    def test_ci_search_list_can_restore_empty_original_list(self):
        script = '''
security() {
  if [[ "$*" == 'list-keychains -d user' ]]; then return 0; fi
  [[ "$1 $2 $3 $4" == 'list-keychains -d user -s' ]]
  if [[ "$#" == 5 ]]; then [[ "$5" == /temporary/test.keychain-db ]];
  else [[ "$#" == 4 ]]; fi
}
SIGNING_KEYCHAIN=/temporary/test.keychain-db
volt_capture_keychain_search_list
volt_add_signing_keychain
volt_restore_keychain_search_list
'''
        result = subprocess.run(['bash', '-c', f'set -eu; source "{KEYCHAIN}"; {script}'],
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == '__main__':
    unittest.main()
