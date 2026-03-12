# Nightingale Testing Strategy

## Philosophy

Testing serves two purposes:
1. **Catch regressions** — ensure changes don't break existing functionality
2. **Enable human review** — make it easy to visually verify rendering changes

We prioritize **human-in-the-loop visual review** over automated pixel-perfect regression, because music engraving is fundamentally a visual craft where "correct" is often a matter of judgment.

## Inspiration: LilyPond's Regression Tests

LilyPond's testing approach is exemplary: https://lilypond.org/doc/v2.23/input/regression/collated-files.html

Each test is:
- **Focused** — demonstrates one specific feature or edge case
- **Minimal** — contains only what's needed to test that feature
- **Visual** — rendered output is the primary artifact
- **Documented** — includes explanation of what's being tested

We adopt this philosophy: tests should be **small, focused, and visual**.

## Testing Architecture

### Primary Review Tool: Flutter App

The `nightingale/` Flutter app is the **ONLY visual review tool** for engraving changes:

```bash
cd nightingale
flutter run -d macos
```

The app:
- Loads all NGL and Notelist fixtures from `assets/scores/`
- Renders them via Rust → RenderCommand stream → Flutter Canvas
- Allows interactive panning/zooming for detailed inspection
- **This is your source of truth for visual quality**

**Workflow (MANDATORY before committing rendering changes):**
1. Make rendering change in Rust
2. `flutter run` in `nightingale/`
3. Browse ALL affected scores visually
4. Check for collisions, spacing issues, missing elements
5. If correct → commit
6. If wrong → iterate

**Never commit rendering changes without Flutter visual review.**

### Automated Test Suite

Rust tests in `tests/` verify **structural correctness**, not pixel-perfect output:

#### 1. Command Stream Tests (`ngl_all.rs`, `notelist_all.rs`)
- Renders all fixtures to `CommandRenderer`
- Snapshots the command stream with `insta`
- Detects unexpected changes in drawing call sequences
- **Purpose:** Catch unintended side effects in rendering logic

#### 2. PDF Generation Tests (`ngl_all.rs`, `notelist_all.rs`)
- Renders all fixtures to `PdfRenderer`
- Verifies PDF generation doesn't panic
- Checks for obviously broken output (e.g., zero-size pages)
- **Purpose:** Ensure PDF output is valid and non-empty

#### 3. Geometry Tests (`ngl_all.rs`, `notelist_all.rs`)
- Validates basic sanity: stems point up/down, notes are on staff, etc.
- **Purpose:** Catch gross positioning errors

#### 4. Focused Feature Tests (future: `tests/focused/`)
- Small, single-issue tests in LilyPond style
- Each test demonstrates one specific rendering feature
- E.g., `grace_notes_before_barline.nl`, `beam_across_rest.nl`, `ledger_lines_stem_collision.nl`
- **Purpose:** Regression tests for specific bugs/features

### What We Don't Test

- **Pixel-perfect bitmap regression** — music engraving evolves; pixel diffs are too brittle
- **Cross-validation against OG Nightingale** — implementation has diverged too far
- **Exhaustive combinatorial coverage** — real-world scores provide better coverage

## Test Data

### Fixture Priorities

1. **Notelist files** (`tests/notelist_examples/`) — simplest, most portable
2. **NGL fixtures** (`tests/fixtures/`) — real Nightingale documents
3. **Focused test cases** (future: `tests/focused/`) — minimal examples per issue

### Adding New Tests

When fixing a bug or adding a feature:

1. **Create a minimal Notelist file** demonstrating the issue
   - Place in `tests/notelist_examples/`
   - Name clearly: `bug_123_grace_note_collision.nl`
   - Add a comment at the top explaining what's being tested

2. **Add to Flutter app assets** (optional, for visual review)
   - Copy to `nightingale/assets/scores/`
   - Add entry in `nightingale/lib/main.dart` `_bundledScores` list

3. **Run automated tests** to verify the fix doesn't break other things
   ```bash
   cargo test --test ngl_all
   cargo test --test notelist_all
   ```

## Running Tests

### Full test suite (structural validation only)
```bash
cargo test
```

### Update command stream snapshots after intentional changes
```bash
INSTA_UPDATE=always cargo test
```

### Run tests for a specific fixture
```bash
cargo test --test ngl_all test_all_ngl_regression_snapshots -- 01_me_and_lucy --nocapture
```

### Visual review (human inspection)
```bash
cd nightingale
flutter run -d macos
```

## CI/CD

The pre-commit hook runs the full test suite. Don't run `cargo test --all` manually before committing — the hook already does it.

CI should:
1. Run `cargo test` (structural validation)
2. Run `cargo clippy` and `cargo fmt --check`
3. **Not** run bitmap regression (we don't have bitmap goldens anymore)

## Principles

1. **Tests should enable change, not prevent it** — brittle tests that break on every rendering tweak are worse than no tests.

2. **Visual review is king** — automated tests catch gross errors; human eyes verify correct engraving.

3. **Real-world scores beat synthetic tests** — Geoff's 17 songs and the classical repertoire Notelists provide better coverage than hand-crafted edge cases.

4. **One test per issue** — when you fix a bug, add a minimal test demonstrating that specific bug, LilyPond-style.

5. **Delete tests that don't pull their weight** — if a test is flaky, unclear, or redundant, remove it.

## Migration Notes

Previous testing approach had multiple overlapping layers:
- Bitmap pixel-level regression (700+ golden PNGs)
- Command stream snapshots
- PDF generation
- Cross-validation against OG Nightingale
- Canonical comparison against known-good PDFs

This was expensive to maintain and brittle. The new approach is:
- **Command stream snapshots** (structural validation)
- **PDF generation** (smoke test)
- **Flutter app visual review** (human judgment)
- **Focused LilyPond-style tests** (regression insurance for specific bugs)

Files moved to `icebox/` during cleanup:
- `tests/og_comparison.rs` — OG Nightingale diverged too far
- `tests/cross_validate.rs` — unclear value
- `tests/canonical_comparison.rs` — redundant with visual review
- `tests/golden_diff.rs` — bitmap-specific, no longer needed
- `tests/golden_bitmaps/` — 700+ PNG files, replaced by Flutter app review

## Future Directions

1. **Expand focused test suite** — add LilyPond-style minimal examples to `tests/focused/`
2. **Automate Flutter screenshot capture** — could snapshot Flutter-rendered PNGs for regression, but keep it opt-in
3. **MusicXML roundtrip tests** — import → export → import should be idempotent
4. **SMuFL compliance tests** — verify glyph rendering matches SMuFL spec
