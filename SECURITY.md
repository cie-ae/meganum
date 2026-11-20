# Security Policy

## Reporting a vulnerability

Report suspected vulnerabilities privately by email to
**security@meganum.dev**.

- Use private disclosure only. Do not file public GitHub issues, pull
  requests, or discussions for security reports.
- Include a description of the issue, steps to reproduce, affected
  versions, and any proof-of-concept code.
- Please keep reports confidential until a fix is released and coordinated.

## Response targets

| Stage | Target |
| --- | --- |
| Acknowledgment of your report | within 2 business days |
| Initial assessment (severity, affected versions, plan) | within 7 days |

If the assessment is delayed, we will say so and keep you updated until a
fix or mitigation ships.

## Supported versions

| Version | Supported |
| --- | --- |
| 1.0.x | Yes |

Earlier development releases are not supported; upgrade to the latest 1.0.x
patch release before reporting.

## Scope

MegaNum is a general-purpose arbitrary-precision arithmetic library.

- **No cryptographic constant-time guarantees.** MegaNum does not aim to be
  constant-time, does not defend against side-channel or timing attacks,
  and is not a substitute for a vetted cryptographic library.
- Numbers derived from secret material (keys, nonces) should not be
  processed with MegaNum in settings where timing behavior matters.

## Safe harbor

We consider good-faith security research to be a legitimate contribution.
We will not pursue legal action against anyone who researches,
discloses responsibly, and does not disrupt service or access data beyond
the scope of their own accounts and systems.
