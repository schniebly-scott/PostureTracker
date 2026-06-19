# Windows SmartScreen flags the app as untrusted on launch ("Run anyway")

**Severity: medium (distribution/trust, Windows-only) — first-run users hit a blue
"Windows protected your PC" SmartScreen dialog and must click *More info → Run anyway*.
No code defect; it's a code-signing / reputation problem, but it materially hurts
install conversion and looks like malware to non-technical users.**

## Where
- Not a source-code issue. It's a property of the distributed `.exe`: the binary is
  **unsigned** (no Authenticode signature) and has **no SmartScreen reputation**, so
  Microsoft Defender SmartScreen warns on download/launch.
- Relevant to the build/release pipeline, not `src/`. See the existing release workflow
  referenced in git history ("New workflow for automated release on version bump",
  commit `2366d45`) — that's where signing would slot in.

## Why this happens
Windows SmartScreen gates execution of downloaded executables on two things:
1. **Authenticode signature** — is the binary signed by a known publisher whose identity
   was verified by a CA? An unsigned `cargo build` artifact is not.
2. **Reputation** — even signed binaries earn trust only after enough machines have run
   them without incident. A brand-new binary (or one re-signed with a fresh cert) has no
   reputation yet, so it's treated as "unknown publisher."

Because our release artifact is an unsigned freshly-built `.exe` pulled from the internet
(it carries the Mark-of-the-Web after download), SmartScreen shows the "unrecognized app"
prompt. This is expected behavior for any unsigned indie binary — nothing in our Rust
code can suppress it.

## Options (in rough order of cost/effectiveness)
1. **Standard ("OV") Authenticode code-signing certificate** (~$100–300/yr from a CA).
   Sign the `.exe` (and installer, if any) in the release workflow with `signtool`.
   Removes the "unknown publisher" framing, but a new OV cert still has to *accumulate*
   SmartScreen reputation, so early downloads may still warn for a while.
2. **EV ("Extended Validation") code-signing certificate** (pricier, hardware-token /
   cloud-HSM backed). EV signatures get **immediate** SmartScreen reputation — the prompt
   is gone from day one. This is the only option that fully removes the warning right
   away. Best choice if a steady stream of Windows users is expected.
3. **Ship through a store/package manager** (Microsoft Store, `winget`, Chocolatey).
   These carry their own trust chain and largely sidestep the raw-`.exe` SmartScreen
   path; more distribution overhead, and the Store has its own packaging requirements.
4. **Document the workaround** as a stopgap: a short "first launch on Windows" note in
   the README/release notes explaining the *More info → Run anyway* path and the
   right-click *Properties → Unblock* option. Does not remove the warning; just lowers
   user anxiety until signing is in place.

## Suggested direction
- Short term: add the README/release-notes note (option 4) so users aren't scared off.
- Medium term: budget for an **EV certificate** (option 2) and wire `signtool` signing
  into the existing version-bump release workflow so every published artifact is signed
  before upload. EV is what actually makes the prompt disappear immediately; an OV cert
  only helps after reputation builds.
- Whatever cert is chosen, sign in CI from a secret-stored key/token, and verify the
  signature on the published artifact as a release step.

## Acceptance check
Download a freshly-released signed `.exe` on a clean Windows machine (one that has never
run the app) and confirm it launches without the SmartScreen "Windows protected your PC"
dialog — or, with an OV cert, that the prompt names the verified publisher rather than
"unknown publisher."
