// QA Compare screen: side-by-side comparison of before/after PNG rendering deltas.
//
// Loads before/after PNG pairs from test-output/qa-compare/ and displays them
// with multiple comparison modes (side-by-side, overlay/blink, slider).

import 'dart:io';
import 'dart:typed_data';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart' show KeyDownEvent, LogicalKeyboardKey;

/// Converts image bytes to a Flutter [ui.Image] for display.
Future<ui.Image> bytesToImage(Uint8List bytes, int width, int height) async {
  final completer = await ui.ImmutableBuffer.fromUint8List(bytes);
  final descriptor = ui.ImageDescriptor.raw(
    completer,
    width: width,
    height: height,
    pixelFormat: ui.PixelFormat.rgba8888,
  );
  final codec = await descriptor.instantiateCodec();
  final frame = await codec.getNextFrame();
  return frame.image;
}

/// View modes for comparison.
enum CompareMode { sideBySide, blink, slider, diffOnly }

/// Information about a before/after QA compare fixture.
class QaFixtureInfo {
  final String fixtureName;
  final File beforeFile;
  final File afterFile;

  QaFixtureInfo({
    required this.fixtureName,
    required this.beforeFile,
    required this.afterFile,
  });
}

/// Main QA Compare screen widget.
class QaCompareScreen extends StatefulWidget {
  final String projectRoot;
  const QaCompareScreen({super.key, required this.projectRoot});

  @override
  State<QaCompareScreen> createState() => _QaCompareScreenState();
}

class _QaCompareScreenState extends State<QaCompareScreen> {
  List<QaFixtureInfo>? _fixtures;
  int _selectedIdx = -1;
  CompareMode _mode = CompareMode.sideBySide;
  bool _loading = false;
  String _status = 'Loading fixtures...';

  // Comparison images
  ui.Image? _beforeImage;
  ui.Image? _afterImage;

  // Blink animation
  bool _blinkShowAfter = false;

  // Slider position (0.0 = all before, 1.0 = all after)
  double _sliderPos = 0.5;

  @override
  void initState() {
    super.initState();
    _loadFixtures();
  }

  Future<void> _loadFixtures() async {
    debugPrint('[QA Compare] loadFixtures: projectRoot=${widget.projectRoot}');
    try {
      if (widget.projectRoot.isEmpty) {
        setState(() => _status = 'Project root not found. Run from project directory.');
        return;
      }

      final qaCompareDir = Directory('${widget.projectRoot}/test-output/qa-compare');
      final beforeDir = Directory('${qaCompareDir.path}/before');
      final afterDir = Directory('${qaCompareDir.path}/after');
      final changedFile = File('${qaCompareDir.path}/changed.txt');

      debugPrint('[QA Compare] Checking for changed.txt at: ${changedFile.path}');

      if (!changedFile.existsSync()) {
        setState(() => _status = 'No changed fixtures found. Run ./scripts/qa-compare-smart.sh first.');
        return;
      }

      // Read changed.txt manifest to get only fixtures with deltas
      final changedContent = await changedFile.readAsString();

      if (changedContent.trim().isEmpty) {
        setState(() => _status = 'No visual changes detected. All fixtures match.');
        return;
      }

      final fixtures = <QaFixtureInfo>[];
      final lines = changedContent.trim().split('\n');

      // Format: name|pct|diff_px/total_px
      for (final line in lines) {
        final parts = line.split('|');
        if (parts.isEmpty) continue;

        final name = parts[0];
        final beforeFile = File('${beforeDir.path}/$name.png');
        final afterFile = File('${afterDir.path}/$name.png');

        if (beforeFile.existsSync() && afterFile.existsSync()) {
          fixtures.add(QaFixtureInfo(
            fixtureName: name,
            beforeFile: beforeFile,
            afterFile: afterFile,
          ));
        }
      }

      debugPrint('[QA Compare] Loaded ${fixtures.length} changed fixtures from manifest');

      setState(() {
        _fixtures = fixtures;
        _status = '${fixtures.length} changed fixture(s)';
      });

      if (fixtures.isNotEmpty) {
        _selectFixture(0);
      }
    } catch (e, st) {
      debugPrint('[QA Compare] Error loading fixtures: $e\n$st');
      setState(() => _status = 'Error loading fixtures: $e');
    }
  }

  Future<void> _selectFixture(int idx) async {
    if (_fixtures == null || idx < 0 || idx >= _fixtures!.length) return;
    setState(() {
      _selectedIdx = idx;
      _loading = true;
      _beforeImage = null;
      _afterImage = null;
    });
    await _loadComparison();
  }

  Future<void> _loadComparison() async {
    if (_fixtures == null || _selectedIdx < 0) return;
    final fixture = _fixtures![_selectedIdx];

    setState(() {
      _loading = true;
      _status = 'Loading ${fixture.fixtureName}...';
    });

    try {
      // Load before image
      final beforeBytes = await fixture.beforeFile.readAsBytes();
      final beforeImage = await decodeImageFromList(beforeBytes);

      // Load after image
      final afterBytes = await fixture.afterFile.readAsBytes();
      final afterImage = await decodeImageFromList(afterBytes);

      setState(() {
        _beforeImage = beforeImage;
        _afterImage = afterImage;
        _loading = false;
        _status = '${fixture.fixtureName}  '
            'Before: ${beforeImage.width}x${beforeImage.height}  '
            'After: ${afterImage.width}x${afterImage.height}';
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
          // Fixture sidebar
          SizedBox(
            width: 240,
            child: Column(
              children: [
                Container(
                  padding: const EdgeInsets.fromLTRB(16, 12, 16, 12),
                  decoration: BoxDecoration(color: colorScheme.primaryContainer),
                  child: Row(
                    children: [
                      Icon(Icons.compare, color: colorScheme.onPrimaryContainer),
                      const SizedBox(width: 8),
                      Text('QA Compare',
                          style: TextStyle(
                              fontSize: 16,
                              fontWeight: FontWeight.w600,
                              color: colorScheme.onPrimaryContainer)),
                    ],
                  ),
                ),
                Expanded(
                  child: _fixtures == null
                      ? const Center(child: CircularProgressIndicator())
                      : _fixtures!.isEmpty
                          ? const Center(
                              child: Text('No QA compare fixtures found',
                                  style: TextStyle(fontSize: 12)))
                          : ListView.builder(
                              itemCount: _fixtures!.length,
                              itemBuilder: (context, i) {
                                final f = _fixtures![i];
                                final selected = i == _selectedIdx;
                                return ListTile(
                                  dense: true,
                                  visualDensity: VisualDensity.compact,
                                  selected: selected,
                                  selectedTileColor:
                                      colorScheme.primaryContainer.withValues(alpha: 0.5),
                                  title: Text(f.fixtureName,
                                      style: TextStyle(
                                          fontSize: 13,
                                          fontWeight: selected
                                              ? FontWeight.w600
                                              : FontWeight.normal)),
                                  onTap: () => _selectFixture(i),
                                );
                              },
                            ),
                ),
              ],
            ),
          ),
          VerticalDivider(width: 1, color: colorScheme.outlineVariant),

          // Main comparison area
          Expanded(
            child: Column(
              children: [
                // Toolbar
                _buildToolbar(colorScheme),
                // Status bar
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 6),
                  decoration: BoxDecoration(
                    color: colorScheme.surfaceContainerLow,
                    border: Border(bottom: BorderSide(color: colorScheme.outlineVariant)),
                  ),
                  child: Row(
                    children: [
                      if (_loading)
                        const SizedBox(
                            width: 14,
                            height: 14,
                            child: CircularProgressIndicator(strokeWidth: 2)),
                      if (_loading) const SizedBox(width: 8),
                      Expanded(
                        child: Text(_status,
                            style: TextStyle(fontSize: 12, color: colorScheme.onSurfaceVariant)),
                      ),
                    ],
                  ),
                ),
                // Comparison view
                Expanded(child: _buildComparisonView()),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildToolbar(ColorScheme colorScheme) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      decoration: BoxDecoration(
        color: colorScheme.surfaceContainerHighest,
        border: Border(bottom: BorderSide(color: colorScheme.outlineVariant)),
      ),
      child: Row(
        children: [
          // View mode toggle
          SegmentedButton<CompareMode>(
            segments: const [
              ButtonSegment(value: CompareMode.sideBySide, label: Text('Side by Side')),
              ButtonSegment(value: CompareMode.blink, label: Text('Blink')),
              ButtonSegment(value: CompareMode.slider, label: Text('Slider')),
            ],
            selected: {_mode},
            onSelectionChanged: (s) => setState(() => _mode = s.first),
            style: ButtonStyle(
              visualDensity: VisualDensity.compact,
              textStyle: WidgetStateProperty.all(const TextStyle(fontSize: 12)),
            ),
          ),
          const Spacer(),
          // Back to score browser
          TextButton.icon(
            onPressed: () => Navigator.of(context).pop(),
            icon: const Icon(Icons.arrow_back, size: 16),
            label: const Text('Scores', style: TextStyle(fontSize: 12)),
          ),
        ],
      ),
    );
  }

  Widget _buildComparisonView() {
    if (_beforeImage == null || _afterImage == null) {
      if (_loading) {
        return const Center(child: CircularProgressIndicator());
      }
      return const Center(child: Text('Select a fixture to compare'));
    }

    switch (_mode) {
      case CompareMode.sideBySide:
        return _buildSideBySide();
      case CompareMode.blink:
        return _buildBlink();
      case CompareMode.slider:
        return _buildSlider();
      case CompareMode.diffOnly:
        // For now, show side-by-side; diffOnly would require pixel-level comparison
        return _buildSideBySide();
    }
  }

  Widget _buildSideBySide() {
    return Container(
      color: Colors.grey.shade300,
      child: SingleChildScrollView(
        child: SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                _imageColumn('Before', _beforeImage!),
                const SizedBox(width: 8),
                _imageColumn('After', _afterImage!),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _imageColumn(String label, ui.Image img) {
    return Column(
      children: [
        Text(label,
            style: TextStyle(
                fontSize: 11,
                color: Colors.grey.shade600,
                fontWeight: FontWeight.w600,
                letterSpacing: 0.5)),
        const SizedBox(height: 4),
        Container(
          decoration: BoxDecoration(
            border: Border.all(color: Colors.grey.shade400, width: 0.5),
            boxShadow: [BoxShadow(color: Colors.black26, blurRadius: 4, offset: const Offset(2, 2))],
          ),
          child: RawImage(image: img, filterQuality: FilterQuality.none),
        ),
      ],
    );
  }

  Widget _buildBlink() {
    final img = _blinkShowAfter ? _afterImage! : _beforeImage!;
    final label = _blinkShowAfter ? 'After' : 'Before';
    return Column(
      children: [
        // Toggle bar
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 6),
          color: Colors.grey.shade200,
          child: Row(
            children: [
              Text(label,
                  style: TextStyle(
                      fontSize: 12,
                      fontWeight: FontWeight.w600,
                      color: Colors.grey.shade700)),
              const Spacer(),
              const Text('Click image or press Space to toggle',
                  style: TextStyle(fontSize: 11, color: Colors.grey)),
            ],
          ),
        ),
        // Scrollable image area
        Expanded(
          child: GestureDetector(
            onTap: () => setState(() => _blinkShowAfter = !_blinkShowAfter),
            child: Focus(
              autofocus: true,
              onKeyEvent: (node, event) {
                if (event is KeyDownEvent &&
                    event.logicalKey == LogicalKeyboardKey.space) {
                  setState(() => _blinkShowAfter = !_blinkShowAfter);
                  return KeyEventResult.handled;
                }
                return KeyEventResult.ignored;
              },
              child: Container(
                color: Colors.grey.shade300,
                child: SingleChildScrollView(
                  child: SingleChildScrollView(
                    scrollDirection: Axis.horizontal,
                    child: Padding(
                      padding: const EdgeInsets.all(16),
                      child: RawImage(
                        image: img,
                        filterQuality: FilterQuality.none,
                      ),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }

  Widget _buildSlider() {
    // Curtain-wipe: clip before image at _sliderPos fraction from left,
    // show after image underneath.
    final beforeW = _beforeImage!.width.toDouble();
    final beforeH = _beforeImage!.height.toDouble();
    final afterW = _afterImage!.width.toDouble();
    final afterH = _afterImage!.height.toDouble();
    final canvasW = beforeW > afterW ? beforeW : afterW;
    final canvasH = beforeH > afterH ? beforeH : afterH;
    final curtainX = canvasW * _sliderPos;

    return Column(
      children: [
        // Slider control
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 6),
          color: Colors.grey.shade200,
          child: Row(
            children: [
              Text('Before',
                  style: TextStyle(
                      fontSize: 11,
                      fontWeight: FontWeight.w600,
                      color: Colors.blue.shade700)),
              Expanded(
                child: Slider(
                  value: _sliderPos,
                  onChanged: (v) => setState(() => _sliderPos = v),
                ),
              ),
              Text('After',
                  style: TextStyle(
                      fontSize: 11,
                      fontWeight: FontWeight.w600,
                      color: Colors.orange.shade700)),
            ],
          ),
        ),
        // Scrollable curtain-wipe area
        Expanded(
          child: Container(
            color: Colors.grey.shade300,
            child: SingleChildScrollView(
              child: SingleChildScrollView(
                scrollDirection: Axis.horizontal,
                child: Padding(
                  padding: const EdgeInsets.all(16),
                  child: SizedBox(
                    width: canvasW,
                    height: canvasH,
                    child: Stack(
                      children: [
                        // After image (full, underneath)
                        Positioned.fill(
                          child: RawImage(
                            image: _afterImage!,
                            filterQuality: FilterQuality.none,
                            alignment: Alignment.topLeft,
                          ),
                        ),
                        // Before image (clipped from left to curtain position)
                        Positioned.fill(
                          child: ClipRect(
                            clipper: _CurtainClipper(curtainX),
                            child: RawImage(
                              image: _beforeImage!,
                              filterQuality: FilterQuality.none,
                              alignment: Alignment.topLeft,
                            ),
                          ),
                        ),
                        // Curtain line
                        Positioned(
                          left: curtainX - 1,
                          top: 0,
                          bottom: 0,
                          child: Container(
                            width: 2,
                            color: Colors.red.shade600,
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

/// Custom clipper for the curtain-wipe slider effect.
/// Clips to a rectangle from the left edge to [curtainX].
class _CurtainClipper extends CustomClipper<Rect> {
  final double curtainX;
  _CurtainClipper(this.curtainX);

  @override
  Rect getClip(Size size) => Rect.fromLTRB(0, 0, curtainX, size.height);

  @override
  bool shouldReclip(_CurtainClipper oldClipper) => oldClipper.curtainX != curtainX;
}
