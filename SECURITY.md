# Security policy

Volt is pre-1.0 software. Security fixes are supported on the latest release;
older versions may not receive backports.

Please do **not** put vulnerabilities, credentials, or reproduction data
containing secrets in public issues or pull requests. Use GitHub's private
vulnerability reporting for this repository once it is enabled. If that option
is unavailable, contact the maintainer through an existing private channel
before disclosing details publicly.

Volt's macOS Secure Event Input indicator means the OS accepted a request to
limit ordinary keyboard event monitoring. It does **not** mask text printed by
the program running in the terminal, erase shell history, protect the system
clipboard, or guarantee that a prompt is a password prompt. Automatic
activation uses a PTY echo-mode heuristic; use the manual toggle for prompts
it cannot detect.
