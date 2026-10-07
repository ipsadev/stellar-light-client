# Security

This contract runs inside other chains' consensus, so a flaw in it can move funds on every chain
that stores it. Please report vulnerabilities privately.

## Reporting

Use GitHub's private vulnerability reporting: **Security → Report a vulnerability** on this
repository. Do not open a public issue, pull request or discussion for a suspected vulnerability.

Include the affected version or commit, the checksum of the wasm if you tested a built artifact, and
the steps or inputs that reproduce the problem.

## What to expect

- an acknowledgement within 3 business days;
- an assessment, and a fix plan or a reasoned rejection, within 14 days;
- a coordinated disclosure date agreed with you, after chains running an affected checksum have had
  time to migrate.

## Scope

In scope: this repository's contract, its release build and the published artifacts. Consensus
verification bugs belong to
[`stellar-consensus-verifier`](https://github.com/ipsadev/stellar-consensus-verifier), but reporting
them here is fine.

Out of scope: chains' own `08-wasm` configuration, and relayers or other software that submits
messages to the client.
