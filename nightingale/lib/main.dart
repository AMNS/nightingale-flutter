import 'dart:io' show Directory, Platform;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart' show rootBundle;
import 'src/rust/api/score.dart';
import 'src/rust/frb_generated.dart';
import 'score_painter.dart';
import 'compare_screen.dart';

Future<void> main() async {
  await RustLib.init();
  runApp(const NightingaleApp());
}

class NightingaleApp extends StatelessWidget {
  const NightingaleApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Nightingale',
      theme: ThemeData(
        colorSchemeSeed: Colors.indigo,
        useMaterial3: true,
        brightness: Brightness.light,
      ),
      darkTheme: ThemeData(
        colorSchemeSeed: Colors.indigo,
        useMaterial3: true,
        brightness: Brightness.dark,
      ),
      home: const ScoreBrowser(),
    );
  }
}

// ── Bundled score catalog ──────────────────────────────────────────

/// A bundled score entry with a display title and asset path.
class _BundledScore {
  final String title;
  final String assetPath;
  final String format; // "ngl" or "nl"

  const _BundledScore({
    required this.title,
    required this.assetPath,
    required this.format,
  });
}

/// All bundled scores, grouped by format.
/// NGL files are loaded as bytes; Notelist files as UTF-8 text.
const List<_BundledScore> _bundledScores = [
  // ── NGL fixtures (Geoff's songs) ──
  _BundledScore(title: 'Me and Lucy', assetPath: 'assets/scores/01_me_and_lucy.ngl', format: 'ngl'),
  _BundledScore(title: 'Cloning Frank Blacks', assetPath: 'assets/scores/02_cloning_frank_blacks.ngl', format: 'ngl'),
  _BundledScore(title: 'Holed Up in Penjinskya', assetPath: 'assets/scores/03_holed_up_in_penjinskya.ngl', format: 'ngl'),
  _BundledScore(title: 'Eating Humble Pie', assetPath: 'assets/scores/04_eating_humble_pie.ngl', format: 'ngl'),
  _BundledScore(title: 'Abigail', assetPath: 'assets/scores/05_abigail.ngl', format: 'ngl'),
  _BundledScore(title: 'Melyssa with a Y', assetPath: 'assets/scores/06_melyssa_with_a_y.ngl', format: 'ngl'),
  _BundledScore(title: 'New York Debutante', assetPath: 'assets/scores/07_new_york_debutante.ngl', format: 'ngl'),
  _BundledScore(title: 'Darling Sunshine', assetPath: 'assets/scores/08_darling_sunshine.ngl', format: 'ngl'),
  _BundledScore(title: 'Swiss Ann', assetPath: 'assets/scores/09_swiss_ann.ngl', format: 'ngl'),
  _BundledScore(title: 'Ghost of Fusion Bob', assetPath: 'assets/scores/10_ghost_of_fusion_bob.ngl', format: 'ngl'),
  _BundledScore(title: 'Philip', assetPath: 'assets/scores/11_philip.ngl', format: 'ngl'),
  _BundledScore(title: 'What Do I Know', assetPath: 'assets/scores/12_what_do_i_know.ngl', format: 'ngl'),
  _BundledScore(title: 'Miss B', assetPath: 'assets/scores/13_miss_b.ngl', format: 'ngl'),
  _BundledScore(title: 'Chrome Molly', assetPath: 'assets/scores/14_chrome_molly.ngl', format: 'ngl'),
  _BundledScore(title: 'Selfsame Twin', assetPath: 'assets/scores/15_selfsame_twin.ngl', format: 'ngl'),
  _BundledScore(title: 'Esmerelda', assetPath: 'assets/scores/16_esmerelda.ngl', format: 'ngl'),
  _BundledScore(title: 'Capital Regiment March', assetPath: 'assets/scores/17_capital_regiment_march.ngl', format: 'ngl'),
  // ── Notelist examples (classical/modern repertoire) ──
  _BundledScore(title: 'Bach: Eb Sonata', assetPath: 'assets/scores/BachEbSonata_20.nl', format: 'nl'),
  _BundledScore(title: 'Bach: St. Anne', assetPath: 'assets/scores/BachStAnne_63.nl', format: 'nl'),
  _BundledScore(title: 'Binchois: De Plus en Plus', assetPath: 'assets/scores/BinchoisDePlus-17.nl', format: 'nl'),
  _BundledScore(title: 'Debussy: Images', assetPath: 'assets/scores/Debussy.Images_9.nl', format: 'nl'),
  _BundledScore(title: 'Goodbye Pork Pie Hat', assetPath: 'assets/scores/GoodbyePorkPieHat.nl', format: 'nl'),
  _BundledScore(title: 'Happy Birthday (multivoice)', assetPath: 'assets/scores/HBD_33.nl', format: 'nl'),
  _BundledScore(title: 'Killing Me Softly', assetPath: 'assets/scores/KillingMe_36.nl', format: 'nl'),
  _BundledScore(title: 'Mahler: Lied von der Erde', assetPath: 'assets/scores/MahlerLiedVonDE_25.nl', format: 'nl'),
  _BundledScore(title: 'Mendelssohn: Op. 7 No. 1', assetPath: 'assets/scores/MendelssohnOp7N1_2.nl', format: 'nl'),
  _BundledScore(title: 'Ravel: Scarbo', assetPath: 'assets/scores/RavelScarbo_15.nl', format: 'nl'),
  _BundledScore(title: 'Schenker: Chopin Diagram', assetPath: 'assets/scores/SchenkerDiagram_Chopin_6.nl', format: 'nl'),
  _BundledScore(title: 'Schoenberg: Op. 19 No. 1', assetPath: 'assets/scores/SchoenbergOp19N1-21.nl', format: 'nl'),
  _BundledScore(title: 'Webern: Op. 5 No. 3', assetPath: 'assets/scores/Webern.Op5N3_22.nl', format: 'nl'),
];

// ── Score browser: sidebar + viewer ──────────────────────────────

class ScoreBrowser extends StatefulWidget {
  const ScoreBrowser({super.key});

  @override
  State<ScoreBrowser> createState() => _ScoreBrowserState();
}

class _ScoreBrowserState extends State<ScoreBrowser> {
  // Score display state
  List<RenderCommandDto>? _commands;
  String _status = 'Select a score';
  bool _loading = false;
  double _scale = 1.5;
  int _selectedIndex = -1;

  @override
  void initState() {
    super.initState();
    // Auto-load the first score
    _loadScore(0);
  }

  Future<void> _loadScore(int index) async {
    final score = _bundledScores[index];
    setState(() {
      _selectedIndex = index;
      _loading = true;
      _status = 'Rendering ${score.title}...';
    });

    try {
      List<RenderCommandDto> commands;
      if (score.format == 'ngl') {
        final data = await rootBundle.load(score.assetPath);
        final bytes = data.buffer.asUint8List();
        debugPrint('[Nightingale] Loading NGL: ${score.assetPath} (${data.lengthInBytes} bytes)');
        commands = await renderNglFromBytes(data: bytes);
      } else {
        // Notelist files are Mac Roman encoded — load as raw bytes and let
        // Rust decode via the encoding-next crate's MAC_ROMAN codec.
        final data = await rootBundle.load(score.assetPath);
        final bytes = data.buffer.asUint8List();
        debugPrint('[Nightingale] Loading Notelist: ${score.assetPath} (${data.lengthInBytes} bytes)');
        commands = await renderNotelistFromBytes(data: bytes);
      }

      debugPrint('[Nightingale] Got ${commands.length} render commands for ${score.title}');

      setState(() {
        _commands = commands;
        _loading = false;
        if (commands.isEmpty) {
          _status = 'Error: no render commands produced for ${score.title}';
        } else {
          int pages = 0;
          for (final cmd in commands) {
            if (cmd.kind == cmdBeginPage) pages++;
          }
          final fmt = score.format.toUpperCase();
          _status = '${score.title}  [$fmt]  |  ${commands.length} commands  |  '
              '${pages > 0 ? pages : 1} page${pages > 1 ? 's' : ''}';
        }
      });
    } catch (e, stackTrace) {
      debugPrint('[Nightingale] Error loading ${score.title}: $e');
      debugPrint('[Nightingale] Stack trace: $stackTrace');
      setState(() {
        _loading = false;
        _status = 'Error loading ${score.title}: $e';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final colorScheme = Theme.of(context).colorScheme;

    return Scaffold(
      body: Row(
        children: [
          // ── Sidebar ─────────────────────────────────────────
          SizedBox(
            width: 260,
            child: Column(
              children: [
                // App header
                Container(
                  padding: const EdgeInsets.fromLTRB(16, 12, 16, 12),
                  decoration: BoxDecoration(
                    color: colorScheme.primaryContainer,
                  ),
                  child: Row(
                    children: [
                      Icon(Icons.music_note, color: colorScheme.onPrimaryContainer),
                      const SizedBox(width: 8),
                      Text(
                        'Nightingale',
                        style: TextStyle(
                          fontSize: 18,
                          fontWeight: FontWeight.w600,
                          color: colorScheme.onPrimaryContainer,
                        ),
                      ),
                    ],
                  ),
                ),

                // Score list
                Expanded(
                  child: ListView.builder(
                    itemCount: _bundledScores.length,
                    itemBuilder: (context, index) {
                      final score = _bundledScores[index];
                      final selected = index == _selectedIndex;

                      // Section headers: NGL before index 0, Notelist before index 17
                      Widget? header;
                      if (index == 0) {
                        header = _sectionHeader('NGL Scores', colorScheme);
                      } else if (index == 17) {
                        header = _sectionHeader('Notelist Scores', colorScheme);
                      }

                      final tile = ListTile(
                        dense: true,
                        visualDensity: VisualDensity.compact,
                        selected: selected,
                        selectedTileColor: colorScheme.primaryContainer.withValues(alpha: 0.5),
                        leading: Icon(
                          score.format == 'ngl' ? Icons.description : Icons.text_snippet,
                          size: 18,
                          color: selected
                              ? colorScheme.primary
                              : score.format == 'ngl'
                                  ? colorScheme.primary.withValues(alpha: 0.6)
                                  : colorScheme.tertiary.withValues(alpha: 0.6),
                        ),
                        title: Text(
                          score.title,
                          style: TextStyle(
                            fontSize: 13,
                            fontWeight: selected ? FontWeight.w600 : FontWeight.normal,
                          ),
                          overflow: TextOverflow.ellipsis,
                        ),
                        onTap: () => _loadScore(index),
                      );

                      if (header != null) {
                        return Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [header, tile],
                        );
                      }
                      return tile;
                    },
                  ),
                ),

                // QA Compare button
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
                  child: SizedBox(
                    width: double.infinity,
                    child: OutlinedButton.icon(
                      icon: const Icon(Icons.compare, size: 16),
                      label: const Text('QA Compare', style: TextStyle(fontSize: 12)),
                      onPressed: () {
                        // Find project root (directory containing Cargo.toml + tests/).
                        // Try current directory first (works in dev mode: flutter run).
                        // If that fails, walk up from executable (works in some release builds).
                        var root = findProjectRoot(startPath: Directory.current.path);
                        if (root.isEmpty) {
                          root = findProjectRoot(startPath: Platform.resolvedExecutable);
                        }
                        if (root.isNotEmpty) {
                          Navigator.of(context).push(
                            MaterialPageRoute(
                              builder: (_) => CompareScreen(projectRoot: root),
                            ),
                          );
                        } else {
                          debugPrint('[QA Compare] Failed to find project root');
                        }
                      },
                    ),
                  ),
                ),

                // Zoom slider
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                  decoration: BoxDecoration(
                    border: Border(top: BorderSide(color: colorScheme.outlineVariant)),
                  ),
                  child: Row(
                    children: [
                      const Text('Zoom', style: TextStyle(fontSize: 12)),
                      Expanded(
                        child: Slider(
                          value: _scale,
                          min: 0.5,
                          max: 4.0,
                          divisions: 14,
                          label: '${(_scale * 100).round()}%',
                          onChanged: (v) => setState(() => _scale = v),
                        ),
                      ),
                      Text('${(_scale * 100).round()}%',
                          style: const TextStyle(fontSize: 11)),
                    ],
                  ),
                ),
              ],
            ),
          ),

          // ── Divider ─────────────────────────────────────────
          VerticalDivider(width: 1, color: colorScheme.outlineVariant),

          // ── Score viewer ────────────────────────────────────
          Expanded(
            child: Column(
              children: [
                // Status bar
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
                  decoration: BoxDecoration(
                    color: colorScheme.surfaceContainerLow,
                    border: Border(
                      bottom: BorderSide(color: colorScheme.outlineVariant),
                    ),
                  ),
                  child: Row(
                    children: [
                      if (_loading)
                        const SizedBox(
                          width: 14, height: 14,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        ),
                      if (_loading) const SizedBox(width: 8),
                      Expanded(
                        child: Text(
                          _status,
                          style: TextStyle(
                            fontSize: 12,
                            color: colorScheme.onSurfaceVariant,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),

                // Score canvas
                Expanded(
                  child: _commands == null
                      ? Center(
                          child: Column(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              Icon(Icons.music_note, size: 48, color: colorScheme.outlineVariant),
                              const SizedBox(height: 16),
                              Text(
                                'Select a score from the sidebar',
                                style: TextStyle(color: colorScheme.outline),
                              ),
                            ],
                          ),
                        )
                      : _commands!.isEmpty
                          ? Center(
                              child: Text(
                                'No render commands produced.\nThe file may be empty or invalid.',
                                textAlign: TextAlign.center,
                                style: TextStyle(color: colorScheme.error),
                              ),
                            )
                          : ScoreView(
                              commands: _commands!,
                              scale: _scale,
                            ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _sectionHeader(String label, ColorScheme colorScheme) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 12, 16, 4),
      child: Text(
        label,
        style: TextStyle(
          fontSize: 11,
          fontWeight: FontWeight.w700,
          color: colorScheme.outline,
          letterSpacing: 0.5,
        ),
      ),
    );
  }
}
