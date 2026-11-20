# Security

This page summarizes the project's security policy; the canonical policy
lives at [`SECURITY.md`](https://github.com/meganum/meganum/blob/main/SECURITY.md)
in the repository root.

## Reporting a vulnerability

Report suspected vulnerabilities privately by email to
**security@meganum.dev**.

- Private disclosure only: do not file public issues, pull requests, or
  discussions for security reports.
- Include a description, reproduction steps, affected versions, and any
  proof-of-concept code.
- Keep reports confidential until a fix is released and coordinated.

## Response targets

| Stage | Target |
| --- | --- |
| Acknowledgment | within 2 business days |
| Initial assessment | within 7 days |

## Supported versions

| Version | Supported |
| --- | --- |
| 1.0.x | Yes |

## Scope

MegaNum makes **no cryptographic constant-time guarantees**. It does not
defend against side-channel or timing attacks and is not a substitute for a
vetted cryptographic library. Avoid processing secret material in settings
where timing behavior matters.

## Safe harbor

Good-faith security research carried out under this policy will not be met
with legal action.
