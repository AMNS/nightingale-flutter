#!/usr/bin/env bash
# make-asset-symlinks.sh — Replace nightingale/assets/scores/ real files with symlinks.
#
# The Flutter app's asset directory (nightingale/assets/scores/) holds symlinks
# pointing to the canonical test fixtures so there is exactly one copy of each
# score file on disk.
#
# Canonical locations:
#   .ngl files  →  tests/fixtures/
#   .nl files   →  tests/notelist_examples/
#
# Git tracks symlinks natively (mode 120000), so after a fresh `git clone` the
# symlinks are recreated automatically. Run this script only if you need to
# recreate them manually (e.g. on a symlink-unfriendly file system).
#
# Usage:
#   scripts/make-asset-symlinks.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SCORES="$REPO_ROOT/nightingale/assets/scores"
# Symlink targets are relative paths from the symlink's location in assets/scores/
FIXTURES_REL="../../../tests/fixtures"
NL_REL="../../../tests/notelist_examples"

mkdir -p "$SCORES"

# NGL fixtures (17 files) — canonical location: tests/fixtures/
for name in \
    01_me_and_lucy 02_cloning_frank_blacks 03_holed_up_in_penjinskya \
    04_eating_humble_pie 05_abigail 06_melyssa_with_a_y 07_new_york_debutante \
    08_darling_sunshine 09_swiss_ann 10_ghost_of_fusion_bob 11_philip \
    12_what_do_i_know 13_miss_b 14_chrome_molly 15_selfsame_twin \
    16_esmerelda 17_capital_regiment_march; do
  ln -sf "${FIXTURES_REL}/${name}.ngl" "$SCORES/${name}.ngl"
done

# Notelist examples (41 files) — canonical location: tests/notelist_examples/
for name in \
    accidentals "BachEbSonata_20.2sizes" BachEbSonata_20 BachStAnne_63 \
    barline_types bass_clef_melody beamed_eighths BinchoisDePlus-17 \
    chord_seconds chromatic_scale clef_change compound_meter \
    Debussy.Images_9 dotted_notes GoodbyePorkPieHat grace_notes_test \
    HBD_33 keysig_d_major keysig_eb_major keysig_flats_all keysig_sharps_all \
    KillingMe_36 ledger_lines MahlerLiedVonDE_25 MendelssohnOp7N1_2 \
    mixed_durations RavelScarbo_15 rests_all_durations SchenkerDiagram_Chopin_6 \
    SchoenbergOp19N1-21 sixteenths_32nds TestMIDIChannels_3 text_annotations \
    tied_notes time_sig_changes tuplet_quintuplet tuplet_triplet two_voices \
    Webern.Op5N3_22 whole_notes wide_intervals; do
  ln -sf "${NL_REL}/${name}.nl" "$SCORES/${name}.nl"
done

echo "Done. Symlinks in $SCORES"
echo "  .ngl → tests/fixtures/            (17 files)"
echo "  .nl  → tests/notelist_examples/   (41 files)"
