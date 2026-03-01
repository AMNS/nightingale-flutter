// QA Compare screen: side-by-side comparison of our rendering vs OG Nightingale.
//
// Provides multiple comparison modes (side-by-side, overlay/blink, slider),
// page navigation, and feedback controls.

import 'dart:typed_data';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart' show KeyDownEvent, LogicalKeyboardKey;
import 'src/rust/api/compare.dart';

/// Converts RGBA bytes to a Flutter [ui.Image] for display.
Future<ui.Image> rgbaToImage(Uint8List rgba, int width, int height) async {
  final completer = await ui.ImmutableBuffer.fromUint8List(rgba);
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

/// Main QA Compare screen widget.
class CompareScreen extends StatefulWidget {
  final String projectRoot;
  const CompareScreen({super.key, required this.projectRoot});

  @override
  State<CompareScreen> createState() => _CompareScreenState();
}

class _CompareScreenState extends State<CompareScreen> {
  List<OgFixtureInfo>? _fixtures;
  int _selectedIdx = -1;
  int _currentPage = 1;
  CompareMode _mode = CompareMode.sideBySide;
  bool _loading = false;
  String _status = 'Loading fixtures...';

  // Comparison result
  ComparisonPageResult? _result;
  ui.Image? _oursImage;
  ui.Image? _ogImage;
  ui.Image? _diffImage;

  // Blink animation
  bool _blinkShowOg = false;

  // Slider position (0.0 = all ours, 1.0 = all OG)
  double _sliderPos = 0.5;

  @override
  void initState() {
    super.initState();
    _loadFixtures();
  }

  Future<void> _loadFixtures() async {
    debugPrint('[QA Compare] loadFixtures: projectRoot=${widget.projectRoot}');
    try {
      final fixtures = await listOgFixtures(projectRoot: widget.projectRoot);
      debugPrint('[QA Compare] got ${fixtures.length} fixtures');
      for (final f in fixtures) {
        debugPrint('[QA Compare]   ${f.fixtureName}: ogExists=${f.ogExists} ogPages=${f.ogPageCount} ourPages=${f.ourPageCount}');
      }
      setState(() {
        _fixtures = fixtures;
        _status = '${fixtures.length} fixtures with OG references';
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
      _currentPage = 1;
      _loading = true;
      _result = null;
      _oursImage = null;
      _ogImage = null;
      _diffImage = null;
    });
    await _loadComparison();
  }

  Future<void> _loadComparison() async {
    if (_fixtures == null || _selectedIdx < 0) return;
    final fixture = _fixtures![_selectedIdx];

    setState(() {
      _loading = true;
      _status = 'Comparing ${fixture.fixtureName} page $_currentPage...';
    });

    try {
      final result = await getComparison(
        projectRoot: widget.projectRoot,
        fixtureName: fixture.fixtureName,
        pageNum: _currentPage,
      );

      if (result.oursWidth == 0 || result.ogWidth == 0) {
        setState(() {
          _loading = false;
          _status = 'Failed to render comparison';
          _result = null;
        });
        return;
      }

      // Convert RGBA to Flutter images
      final oursImg = await rgbaToImage(result.oursRgba, result.oursWidth, result.oursHeight);
      final ogImg = await rgbaToImage(result.ogRgba, result.ogWidth, result.ogHeight);
      final diffImg = await rgbaToImage(result.diffRgba, result.diffWidth, result.diffHeight);

      setState(() {
        _result = result;
        _oursImage = oursImg;
        _ogImage = ogImg;
        _diffImage = diffImg;
        _loading = false;
        _status =
            '${fixture.fixtureName} p$_currentPage — ${result.diffPct.toStringAsFixed(2)}% diff '
            '(${result.diffPixels} / ${result.totalPixels} px)  '
            'Ours: ${result.oursWidth}x${result.oursHeight}  '
            'OG: ${result.ogWidth}x${result.ogHeight}';
      });
    } catch (e) {
      setState(() {
        _loading = false;
        _status = 'Error: $e';
      });
    }
  }

  int get _maxPages {
    if (_fixtures == null || _selectedIdx < 0) return 1;
    final f = _fixtures![_selectedIdx];
    return f.ogPageCount > f.ourPageCount ? f.ogPageCount : f.ourPageCount;
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
                                      fontWeight:
                                          selected ? FontWeight.w600 : FontWeight.normal)),
                              subtitle: Text(
                                  '${f.ogPageCount} pages  |  ${f.ogExists ? "PDF" : "missing"}',
                                  style: const TextStyle(fontSize: 11)),
                              trailing: f.ogExists
                                  ? Icon(Icons.check_circle, size: 16, color: Colors.green.shade300)
                                  : Icon(Icons.error_outline,
                                      size: 16, color: Colors.red.shade300),
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
                      // Diff % badge
                      if (_result != null)
                        Container(
                          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                          decoration: BoxDecoration(
                            color: _diffColor(_result!.diffPct),
                            borderRadius: BorderRadius.circular(4),
                          ),
                          child: Text(
                            '${_result!.diffPct.toStringAsFixed(1)}%',
                            style: const TextStyle(
                                fontSize: 12, fontWeight: FontWeight.bold, color: Colors.white),
                          ),
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
              ButtonSegment(value: CompareMode.diffOnly, label: Text('Diff')),
            ],
            selected: {_mode},
            onSelectionChanged: (s) => setState(() => _mode = s.first),
            style: ButtonStyle(
              visualDensity: VisualDensity.compact,
              textStyle: WidgetStateProperty.all(const TextStyle(fontSize: 12)),
            ),
          ),
          const SizedBox(width: 16),
          // Page navigation
          IconButton(
            icon: const Icon(Icons.chevron_left, size: 20),
            onPressed: _currentPage > 1
                ? () {
                    setState(() => _currentPage--);
                    _loadComparison();
                  }
                : null,
            tooltip: 'Previous page',
          ),
          Text('Page $_currentPage / $_maxPages',
              style: const TextStyle(fontSize: 12)),
          IconButton(
            icon: const Icon(Icons.chevron_right, size: 20),
            onPressed: _currentPage < _maxPages
                ? () {
                    setState(() => _currentPage++);
                    _loadComparison();
                  }
                : null,
            tooltip: 'Next page',
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
    if (_oursImage == null || _ogImage == null || _diffImage == null) {
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
        return _buildDiffOnly();
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
                _imageColumn('Ours (Modern)', _oursImage!),
                const SizedBox(width: 8),
                _imageColumn('Diff', _diffImage!),
                const SizedBox(width: 8),
                _imageColumn('OG (Nightingale)', _ogImage!),
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
    final img = _blinkShowOg ? _ogImage! : _oursImage!;
    final label = _blinkShowOg ? 'OG (Nightingale)' : 'Ours (Modern)';
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
            onTap: () => setState(() => _blinkShowOg = !_blinkShowOg),
            child: Focus(
              autofocus: true,
              onKeyEvent: (node, event) {
                if (event is KeyDownEvent &&
                    event.logicalKey == LogicalKeyboardKey.space) {
                  setState(() => _blinkShowOg = !_blinkShowOg);
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
    // Curtain-wipe: clip our image at _sliderPos fraction from left,
    // show OG image underneath.
    final oursW = _oursImage!.width.toDouble();
    final oursH = _oursImage!.height.toDouble();
    final ogW = _ogImage!.width.toDouble();
    final ogH = _ogImage!.height.toDouble();
    final canvasW = oursW > ogW ? oursW : ogW;
    final canvasH = oursH > ogH ? oursH : ogH;
    final curtainX = canvasW * _sliderPos;

    return Column(
      children: [
        // Slider control
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 6),
          color: Colors.grey.shade200,
          child: Row(
            children: [
              Text('Ours',
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
              Text('OG',
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
                        // OG image (full, underneath)
                        Positioned.fill(
                          child: RawImage(
                            image: _ogImage!,
                            filterQuality: FilterQuality.none,
                            alignment: Alignment.topLeft,
                          ),
                        ),
                        // Our image (clipped from left to curtain position)
                        Positioned.fill(
                          child: ClipRect(
                            clipper: _CurtainClipper(curtainX),
                            child: RawImage(
                              image: _oursImage!,
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

  Widget _buildDiffOnly() {
    return Container(
      color: Colors.grey.shade300,
      child: SingleChildScrollView(
        child: SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: RawImage(image: _diffImage!, filterQuality: FilterQuality.none),
          ),
        ),
      ),
    );
  }

  Color _diffColor(double pct) {
    if (pct < 2.0) return Colors.green.shade700;
    if (pct < 10.0) return Colors.orange.shade700;
    return Colors.red.shade700;
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
