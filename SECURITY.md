# Security policy

## Supported versions

Security fixes are applied to the latest code on the default branch. Older releases may not receive backports.

## Reporting a vulnerability

Please do not open a public issue for a suspected vulnerability and do not include API keys, user databases, private videos or logs containing personal data in a report.

Use the repository's private vulnerability reporting feature under the GitHub **Security** tab. If private reporting is unavailable, contact a maintainer through a private channel listed on their GitHub profile before sharing technical details.

Include the affected version, impact, reproduction conditions and a minimal proof of concept with all credentials and personal data removed. Maintainers will acknowledge the report, assess severity and coordinate disclosure when a fix is available.

## Local credential storage

DramaDNA currently stores provider API keys as plaintext in its local SQLite database. Protect the operating-system account, application data directory and backups accordingly. The application must never log API keys or commit its database to source control.
