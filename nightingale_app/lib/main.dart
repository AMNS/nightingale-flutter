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
/// These paths are resolved by:
/// 1. Checking relative to the current working directory
/// 2. Using the Rust `find_project_root` function to walk up from
///    the executable's location to find the project root
/// 3. Checking common macOS development paths
List<_ScoreDirectory> _defaultDirectories() {
  final candidates = <String>[];

  // From nightingale-modernize/ (project root)
  candidates.add('${Directory.current.path}/tests/fixtures');
  candidates.add('${Directory.current.path}/tests/notelist_examples');

  // From nightingale_app/ subdirectory
  candidates.add('${Directory.current.path}/../tests/fixtures');
  candidates.add('${Directory.current.path}/../tests/notelist_examples');

  // Use Rust helper to find project root from the executable path.
  // This handles the case where the app is launched from Xcode or
  // by double-clicking, where cwd is / or the user's home.
  final exePath = Platform.resolvedExecutable;
  final projectRoot = findProjectRoot(startPath: exePath);
  if (projectRoot.isNotEmpty) {
    candidates.add('$projectRoot/tests/fixtures');
    candidates.add('$projectRoot/tests/notelist_examples');
  }

  // Also try from the script/source directory (for `flutter run` from nightingale_app/)
  final scriptDir = Platform.script.toFilePath();
  final scriptProjectRoot = findProjectRoot(startPath: scriptDir);
  if (scriptProjectRoot.isNotEmpty && scriptProjectRoot != projectRoot) {
    candidates.add('$scriptProjectRoot/tests/fixtures');
    candidates.add('$scriptProjectRoot/tests/notelist_examples');
  }

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

  // Page orientation: false = portrait (default), true = landscape
  bool _landscape = false;

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

  /// Add a directory by path string (typed or pasted).
  void _addDirectoryByPath(String path) {
    final dir = Directory(path);
    if (!dir.existsSync()) return;

    final resolved = dir.resolveSymbolicLinksSync();
    // Check if already in list
    if (_directories.any((d) => d.path == resolved)) {
      // Just select it
      final existing = _directories.firstWhere((d) => d.path == resolved);
      _selectDirectory(existing);
      return;
    }

    final name = resolved.split('/').last;
    final newDir = _ScoreDirectory(name: name, path: resolved);
    setState(() {
      _directories.add(newDir);
      _selectDirectory(newDir);
    });
  }

  /// Show dialog to enter a directory path.
  Future<void> _showOpenDirectoryDialog() async {
    final controller = TextEditingController();
    // Pre-fill with a sensible default
    if (_directories.isNotEmpty) {
      // Go up from the first known directory
      final parent = Directory(_directories.first.path).parent.path;
      controller.text = parent;
    } else {
      controller.text = Directory.current.path;
    }

    final path = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Open Score Directory'),
        content: SizedBox(
          width: 500,
          child: TextField(
            controller: controller,
            autofocus: true,
            decoration: const InputDecoration(
              hintText: '/path/to/directory/with/.ngl/.nl files',
              border: OutlineInputBorder(),
              helperText: 'Enter full path to a directory containing .ngl or .nl files',
            ),
            onSubmitted: (v) => Navigator.of(context).pop(v),
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(controller.text),
            child: const Text('Open'),
          ),
        ],
      ),
    );

    if (path != null && path.isNotEmpty) {
      _addDirectoryByPath(path);
    }
  }

  Future<void> _loadFile(ScoreFileEntry file) async {
    setState(() {
      _selectedFile = file;
      _loading = true;
      _status = 'Rendering ${file.name}...';
    });

    try {
      // Use landscape-aware rendering for Notelist files
      final commands = await renderScoreFromPathLandscape(
        path: file.path,
        landscape: _landscape,
      );
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
          final orientLabel = _landscape ? ' (landscape)' : '';
          _status = '${file.name}$orientLabel  |  ${commands.length} commands  |  '
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

  /// Toggle landscape/portrait and re-render current file if loaded.
  void _toggleOrientation() {
    setState(() {
      _landscape = !_landscape;
    });
    // Re-render current file with new orientation
    if (_selectedFile != null) {
      _loadFile(_selectedFile!);
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
                      // Open directory button
                      IconButton(
                        icon: const Icon(Icons.folder_open, size: 20),
                        tooltip: 'Open score directory',
                        onPressed: _showOpenDirectoryDialog,
                      ),
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
                    child: Column(
                      children: [
                        Text(
                          'No test fixture directories found.',
                          style: TextStyle(color: colorScheme.error, fontSize: 12),
                        ),
                        const SizedBox(height: 8),
                        FilledButton.icon(
                          icon: const Icon(Icons.folder_open, size: 16),
                          label: const Text('Open Directory'),
                          onPressed: _showOpenDirectoryDialog,
                        ),
                      ],
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

                // Orientation + Zoom controls
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                  decoration: BoxDecoration(
                    border: Border(top: BorderSide(color: colorScheme.outlineVariant)),
                  ),
                  child: Column(
                    children: [
                      // Orientation toggle
                      Row(
                        children: [
                          const Text('Page', style: TextStyle(fontSize: 12)),
                          const SizedBox(width: 8),
                          SegmentedButton<bool>(
                            segments: const [
                              ButtonSegment(
                                value: false,
                                icon: Icon(Icons.crop_portrait, size: 16),
                                label: Text('Portrait', style: TextStyle(fontSize: 11)),
                              ),
                              ButtonSegment(
                                value: true,
                                icon: Icon(Icons.crop_landscape, size: 16),
                                label: Text('Landscape', style: TextStyle(fontSize: 11)),
                              ),
                            ],
                            selected: {_landscape},
                            onSelectionChanged: (v) {
                              if (v.first != _landscape) _toggleOrientation();
                            },
                            style: ButtonStyle(
                              visualDensity: VisualDensity.compact,
                              tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                            ),
                          ),
                        ],
                      ),
                      const SizedBox(height: 4),
                      // Zoom slider
                      Row(
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
