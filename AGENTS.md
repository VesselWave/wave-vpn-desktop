# Independent application repository

- This repository builds standalone. Do not depend on parent/sibling files.
- Use this repository's Git remote and history for commits and releases.
- Keep GPL-3.0-or-later and the attributions in `docs/PROVENANCE.md`.
- The parent workspace's `old/` contains archived code with unresolved upstream
  licensing. Do not import its backend, tests, packaging or inherited GUI code.
- Only reuse authored UI/assets with recorded provenance. Implement protocol
  behavior from explicitly licensed official references and record those sources.
- Never commit credentials, private profiles, signing keys, local SDK paths,
  reference checkouts or build outputs.
- Run the checks documented in README.md after relevant changes.
