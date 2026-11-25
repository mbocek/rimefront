# Rimefront

Original real-time strategy game in Rust and Bevy. What lives where: [docs/architecture.md](docs/architecture.md);
building, tests and releases: [docs/development.md](docs/development.md).

## Workflow

- Work directly on `main`; no feature branches.
- One commit per feature, made only after `make check` (formatting, Clippy, all tests) passes.
- Push and release only when asked.
- Feature details belong in `docs/*.md`; the README stays a short index.

## Commits

- Commitizen / Conventional Commits format: `type(scope): description`, with a lowercase imperative
  description, for example `fix(economy): stop harvesters jamming on shared lanes`.
- Types: `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `build`, `ci`, `chore`. A release is
  `bump: version 1.3.1 → 1.4.0`.
- Everything that goes into git or GitHub is English: commit messages, tags, release notes, code and doc
  comments.

## Releases

- Bump the version in `Cargo.toml` (and the tag example in `docs/development.md`), commit it as
  `bump: version X → Y`, push, then push the tag `vX.Y.Z`; CI builds the packages and publishes the release.
- Every release gets detailed notes, not only the generated changelog link: what is new for players, grouped
  (gameplay, visuals, sound, performance, fixes), with measured numbers where there are some, and anything
  that breaks (changed maps; old replays only when map generation or the replay format changed, see
  [docs/determinism.md](docs/determinism.md)). Set them with `gh release edit vX.Y.Z --notes-file <file>` once
  the release exists.

## Constraints

- The simulation is deterministic lockstep. Anything that changes simulation results (map generation,
  crystal placement, the shared map RNG, combat or economy rules) changes `tests/fingerprints.txt`: re-record
  with `UPDATE_FINGERPRINTS=1 cargo test --release --test cross_platform` and say so in the commit.
- What a persistent world says to games and what a saved world holds are versioned (`WORLD_PROTOCOL`,
  `STORE_VERSION`); a change to either fails a fixture test until the number is raised and the fixture
  re-recorded, see [docs/compatibility.md](docs/compatibility.md).
- Generated assets come from code: models from `tools/blender` (`make models`), sounds and music from
  `tools/audio/generate_audio.py`. Change the generator and regenerate; never edit the outputs by hand.
- Third-party assets must be CC0 or similarly free and are credited in the README.
- Measure performance claims (CPU time over a capture run, `make capture`) before writing them into docs.
