import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart' show rootBundle;
import 'src/rust/api/score.dart';
import 'src/rust/frb_generated.dart';
import 'score_painter.dart';

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

// ── Score browser: sidebar + viewer ──────────────────────────────

class ScoreBrowser extends StatefulWidget {
  const ScoreBrowser({super.key});

  @override
  State<ScoreBrowser> createState() => _ScoreBrowserState();
}

/// Known directories containing test fixture files.
///
/// These paths are resolved relative to the executable's location
/// to find the nightingale-modernize project root. Falls back to
/// hardcoded absolute paths on macOS during development.
List<_ScoreDirectory> _defaultDirectories() {
  // Find the project root by looking for Cargo.toml relative to the
  // nightingale_app directory. In development, the working directory
  // is typically nightingale_app/ or nightingale-modernize/.
  final candidates = <String>[
    // From nightingale-modernize/ (project root)
    '${Directory.current.path}/tests/fixtures',
    '${Directory.current.path}/tests/notelist_examples',
    // From nightingale_app/ subdirectory
    '${Directory.current.path}/../tests/fixtures',
    '${Directory.current.path}/../tests/notelist_examples',
  ];

  final dirs = <_ScoreDirectory>[];
  for (final path in candidates) {
    final dir = Directory(path);
    if (dir.existsSync()) {
      final name = path.contains('notelist') ? 'Notelists' : 'NGL Fixtures';
      // Resolve to canonical path
      dirs.add(_ScoreDirectory(name: name, path: dir.resolveSymbolicLinksSync()));
    }
  }

  // Deduplicate by resolved path
  final seen = <String>{};
  dirs.removeWhere((d) => !seen.add(d.path));

  return dirs;
}

class _ScoreDirectory {
  final String name;
  final String path;
  const _ScoreDirectory({required this.name, required this.path});
}

class _ScoreBrowserState extends State<ScoreBrowser> {
  // Sidebar state
  List<_ScoreDirectory> _directories = [];
  _ScoreDirectory? _selectedDirectory;
  List<ScoreFileEntry> _files = [];
  ScoreFileEntry? _selectedFile;

  // Score display state
  List<RenderCommandDto>? _commands;
  String _status = 'Select a score file';
  bool _loading = false;
  double _scale = 1.5;

  @override
  void initState() {
    super.initState();
    _directories = _defaultDirectories();
    if (_directories.isNotEmpty) {
      _selectDirectory(_directories.first);
    }
  }

  void _selectDirectory(_ScoreDirectory dir) {
    setState(() {
      _selectedDirectory = dir;
      _files = listScoreFiles(directory: dir.path);
    });
  }

  Future<void> _loadFile(ScoreFileEntry file) async {
    setState(() {
      _selectedFile = file;
      _loading = true;
      _status = 'Rendering ${file.name}...';
    });

    try {
      final commands = await renderScoreFromPath(path: file.path);
      setState(() {
        _commands = commands;
        _loading = false;
        if (commands.isEmpty) {
          _status = 'Error: no render commands produced';
        } else {
          // Count pages
          int pages = 0;
          for (final cmd in commands) {
            if (cmd.kind == cmdBeginPage) pages++;
          }
          _status = '${file.name}  |  ${commands.length} commands  |  '
              '${pages > 0 ? pages : 1} page${pages > 1 ? 's' : ''}';
        }
      });
    } catch (e) {
      setState(() {
        _loading = false;
        _status = 'Error: $e';
      });
    }
  }

  Future<void> _loadBundledAsset() async {
    setState(() {
      _loading = true;
      _status = 'Loading bundled score...';
    });
    try {
      final data = await rootBundle.load('assets/scores/01_me_and_lucy_simple.ngl');
      final bytes = data.buffer.asUint8List();
      final commands = await renderNglFromBytes(data: bytes);
      setState(() {
        _commands = commands;
        _loading = false;
        _selectedFile = null;
        _status = 'Me and Lucy (bundled)  |  ${commands.length} commands';
      });
    } catch (e) {
      setState(() {
        _loading = false;
        _status = 'Error: $e';
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
            width: 280,
            child: Column(
              children: [
                // App header
                Container(
                  padding: const EdgeInsets.fromLTRB(16, 12, 8, 8),
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
                      const Spacer(),
                      // Bundled asset button
                      IconButton(
                        icon: const Icon(Icons.home, size: 20),
                        tooltip: 'Load bundled score',
                        onPressed: _loadBundledAsset,
                      ),
                    ],
                  ),
                ),

                // Directory tabs
                if (_directories.isNotEmpty)
                  SizedBox(
                    height: 40,
                    child: ListView(
                      scrollDirection: Axis.horizontal,
                      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                      children: _directories.map((dir) {
                        final selected = dir.path == _selectedDirectory?.path;
                        return Padding(
                          padding: const EdgeInsets.only(right: 4),
                          child: FilterChip(
                            label: Text(dir.name, style: const TextStyle(fontSize: 12)),
                            selected: selected,
                            onSelected: (_) => _selectDirectory(dir),
                          ),
                        );
                      }).toList(),
                    ),
                  ),

                if (_directories.isEmpty)
                  Padding(
                    padding: const EdgeInsets.all(16),
                    child: Text(
                      'No test fixture directories found.\n'
                      'Run from nightingale-modernize/ root.',
                      style: TextStyle(color: colorScheme.error, fontSize: 12),
                    ),
                  ),

                // File list
                Expanded(
                  child: ListView.builder(
                    itemCount: _files.length,
                    itemBuilder: (context, index) {
                      final file = _files[index];
                      final selected = file.path == _selectedFile?.path;
                      return ListTile(
                        dense: true,
                        visualDensity: VisualDensity.compact,
                        selected: selected,
                        leading: Icon(
                          file.format == 'ngl' ? Icons.description : Icons.text_snippet,
                          size: 18,
                          color: file.format == 'ngl'
                              ? colorScheme.primary
                              : colorScheme.tertiary,
                        ),
                        title: Text(
                          file.name,
                          style: const TextStyle(fontSize: 13),
                          overflow: TextOverflow.ellipsis,
                        ),
                        subtitle: Text(
                          file.format.toUpperCase(),
                          style: TextStyle(fontSize: 10, color: colorScheme.outline),
                        ),
                        onTap: () => _loadFile(file),
                      );
                    },
                  ),
                ),

                // Zoom controls
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
                                'Select a score file from the sidebar',
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
}
