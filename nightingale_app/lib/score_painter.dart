// ScorePainter: a Flutter CustomPainter that draws a Nightingale score
// from the flat RenderCommandDto list produced by the Rust core.
//
// 32 command types matching the RenderCommand enum variants.
// See CMD_* constants in rust/src/api/score.rs.

import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'src/rust/api/score.dart';

// ── Command kind constants (must match Rust CMD_* constants) ──────────

const int cmdLine = 1;
const int cmdLineVerticalThick = 2;
const int cmdLineHorizontalThick = 3;
const int cmdHDashedLine = 4;
const int cmdVDashedLine = 5;
const int cmdFrameRect = 6;
const int cmdStaffLine = 7;
const int cmdStaff = 8;
const int cmdBarLine = 9;
const int cmdConnectorLine = 10;
const int cmdLedgerLine = 11;
const int cmdRepeatDots = 12;
const int cmdBeam = 13;
const int cmdSlur = 14;
const int cmdBracket = 15;
const int cmdBrace = 16;
const int cmdNoteStem = 17;
const int cmdMusicChar = 18;
const int cmdMusicString = 19;
const int cmdTextString = 20;
const int cmdMusicColon = 21;
const int cmdSetLineWidth = 22;
const int cmdSetWidths = 23;
const int cmdSetMusicSize = 24;
const int cmdSetPageSize = 25;
const int cmdBeginPage = 26;
const int cmdEndPage = 27;
const int cmdSaveState = 28;
const int cmdRestoreState = 29;
const int cmdTranslate = 30;
const int cmdScale = 31;
const int cmdSetColor = 32;

/// Paints a list of [RenderCommandDto]s onto a Flutter [Canvas].
///
/// This replays the Rust render command stream using Flutter's Canvas API
/// and a SMuFL-compatible font (Bravura) for music glyphs.
class ScorePainter extends CustomPainter {
  final List<RenderCommandDto> commands;

  /// Scale factor: typographic points to logical pixels.
  final double scale;

  /// Font family for SMuFL glyphs (e.g. "Bravura").
  final String musicFontFamily;

  /// Font family for text (tempo, lyrics, rehearsal marks).
  final String textFontFamily;

  /// Debug mode: draw diagnostics.
  final bool debug;

  ScorePainter({
    required this.commands,
    this.scale = 1.0,
    this.musicFontFamily = 'Bravura',
    this.textFontFamily = 'Serif',
    this.debug = false,
  });

  // ── State tracking (mirrors Rust render state) ──────────────────

  double _lineWidth = 1.0;
  double _staffLineWidth = 0.8;
  double _ledgerLineWidth = 1.0;
  double _stemLineWidth = 0.8;
  double _barLineWidth = 1.0;
  double _musicSizePercent = 100.0;
  double _currentColorR = 0.0;
  double _currentColorG = 0.0;
  double _currentColorB = 0.0;
  double _currentColorA = 1.0;

  // Transform stack
  final List<_SavedState> _stateStack = [];
  double _translateX = 0.0;
  double _translateY = 0.0;
  double _scaleX = 1.0;
  double _scaleY = 1.0;

  // Multi-page state
  double _pageWidth = 612.0;
  double _pageHeight = 792.0;
  int _currentPage = 0;

  /// Gap between pages in scaled pixels.
  static const double pageGap = 16.0;

  @override
  void paint(Canvas canvas, Size size) {
    // Reset state
    _lineWidth = 1.0;
    _staffLineWidth = 0.8;
    _ledgerLineWidth = 1.0;
    _stemLineWidth = 0.8;
    _barLineWidth = 1.0;
    _musicSizePercent = 100.0;
    _currentColorR = 0.0;
    _currentColorG = 0.0;
    _currentColorB = 0.0;
    _currentColorA = 1.0;
    _stateStack.clear();
    _translateX = 0.0;
    _translateY = 0.0;
    _scaleX = 1.0;
    _scaleY = 1.0;
    _pageWidth = 612.0;
    _pageHeight = 792.0;
    _currentPage = 0;

    if (debug) {
      debugPrint('ScorePainter: painting ${commands.length} commands, scale=$scale');
    }

    for (final cmd in commands) {
      switch (cmd.kind) {
        case cmdLine:
          _drawLine(canvas, cmd);
          break;
        case cmdLineVerticalThick:
        case cmdLineHorizontalThick:
          _drawThickLine(canvas, cmd);
          break;
        case cmdHDashedLine:
          _drawHDashedLine(canvas, cmd);
          break;
        case cmdVDashedLine:
          _drawVDashedLine(canvas, cmd);
          break;
        case cmdFrameRect:
          _drawFrameRect(canvas, cmd);
          break;
        case cmdStaffLine:
          _drawStaffLine(canvas, cmd);
          break;
        case cmdStaff:
          _drawStaff(canvas, cmd);
          break;
        case cmdBarLine:
          _drawBarLine(canvas, cmd);
          break;
        case cmdConnectorLine:
          _drawConnectorLine(canvas, cmd);
          break;
        case cmdLedgerLine:
          _drawLedgerLine(canvas, cmd);
          break;
        case cmdRepeatDots:
          _drawRepeatDots(canvas, cmd);
          break;
        case cmdBeam:
          _drawBeam(canvas, cmd);
          break;
        case cmdSlur:
          _drawSlur(canvas, cmd);
          break;
        case cmdBracket:
          _drawBracket(canvas, cmd);
          break;
        case cmdBrace:
          _drawBrace(canvas, cmd);
          break;
        case cmdNoteStem:
          _drawNoteStem(canvas, cmd);
          break;
        case cmdMusicChar:
          _drawMusicChar(canvas, cmd);
          break;
        case cmdMusicString:
          _drawMusicString(canvas, cmd);
          break;
        case cmdTextString:
          _drawTextString(canvas, cmd);
          break;
        case cmdMusicColon:
          _drawMusicColon(canvas, cmd);
          break;
        case cmdSetLineWidth:
          _lineWidth = cmd.width;
          break;
        case cmdSetWidths:
          _staffLineWidth = cmd.x0;
          _ledgerLineWidth = cmd.x1;
          _stemLineWidth = cmd.x2;
          _barLineWidth = cmd.x3;
          break;
        case cmdSetMusicSize:
          _musicSizePercent = cmd.sizePercent;
          break;
        case cmdSetPageSize:
          _pageWidth = cmd.width > 0 ? cmd.width : 612;
          _pageHeight = cmd.height > 0 ? cmd.height : 792;
          break;
        case cmdBeginPage:
          // Multi-page: offset canvas Y for each page, with a gap between.
          canvas.save();
          final pageY = _currentPage * (_pageHeight * scale + pageGap);
          canvas.translate(0, pageY);
          // Draw page background and shadow
          _drawPageBackground(canvas);
          _currentPage++;
          break;
        case cmdEndPage:
          canvas.restore();
          break;
        case cmdSaveState:
          canvas.save();
          _stateStack.add(_SavedState(
            translateX: _translateX,
            translateY: _translateY,
            scaleX: _scaleX,
            scaleY: _scaleY,
            colorR: _currentColorR,
            colorG: _currentColorG,
            colorB: _currentColorB,
            colorA: _currentColorA,
            lineWidth: _lineWidth,
          ));
          break;
        case cmdRestoreState:
          canvas.restore();
          if (_stateStack.isNotEmpty) {
            final s = _stateStack.removeLast();
            _translateX = s.translateX;
            _translateY = s.translateY;
            _scaleX = s.scaleX;
            _scaleY = s.scaleY;
            _currentColorR = s.colorR;
            _currentColorG = s.colorG;
            _currentColorB = s.colorB;
            _currentColorA = s.colorA;
            _lineWidth = s.lineWidth;
          }
          break;
        case cmdTranslate:
          final dx = cmd.x0 * scale;
          final dy = cmd.y0 * scale;
          canvas.translate(dx, dy);
          _translateX += dx;
          _translateY += dy;
          break;
        case cmdScale:
          canvas.scale(cmd.x0, cmd.y0);
          _scaleX *= cmd.x0;
          _scaleY *= cmd.y0;
          break;
        case cmdSetColor:
          _currentColorR = cmd.colorR;
          _currentColorG = cmd.colorG;
          _currentColorB = cmd.colorB;
          _currentColorA = cmd.colorA;
          break;
      }
    }
  }

  @override
  bool shouldRepaint(covariant ScorePainter oldDelegate) {
    return commands != oldDelegate.commands || scale != oldDelegate.scale;
  }

  // ── Page background ─────────────────────────────────────────────

  void _drawPageBackground(Canvas canvas) {
    final w = _pageWidth * scale;
    final h = _pageHeight * scale;
    // Drop shadow
    canvas.drawRect(
      Rect.fromLTWH(2, 2, w, h),
      Paint()..color = Colors.black.withValues(alpha: 0.08),
    );
    // White page
    canvas.drawRect(
      Rect.fromLTWH(0, 0, w, h),
      Paint()..color = Colors.white,
    );
    // Border
    canvas.drawRect(
      Rect.fromLTWH(0, 0, w, h),
      Paint()
        ..color = Colors.grey.shade300
        ..style = PaintingStyle.stroke
        ..strokeWidth = 0.5,
    );
  }

  // ── Helpers ──────────────────────────────────────────────────────

  Color _color() => Color.fromRGBO(
        (_currentColorR * 255).round().clamp(0, 255),
        (_currentColorG * 255).round().clamp(0, 255),
        (_currentColorB * 255).round().clamp(0, 255),
        _currentColorA.clamp(0.0, 1.0),
      );

  Paint _strokePaint(double strokeWidth) => Paint()
    ..color = _color()
    ..strokeWidth = strokeWidth * scale
    ..style = PaintingStyle.stroke
    ..isAntiAlias = true;

  Paint _fillPaint() => Paint()
    ..color = _color()
    ..style = PaintingStyle.fill
    ..isAntiAlias = true;

  Offset _pt(double x, double y) => Offset(x * scale, y * scale);

  // ── Drawing commands ────────────────────────────────────────────

  void _drawLine(Canvas canvas, RenderCommandDto cmd) {
    canvas.drawLine(
      _pt(cmd.x0, cmd.y0),
      _pt(cmd.x1, cmd.y1),
      _strokePaint(cmd.width > 0 ? cmd.width : _lineWidth),
    );
  }

  void _drawThickLine(Canvas canvas, RenderCommandDto cmd) {
    // Thick lines are drawn as filled rectangles
    final x0 = cmd.x0 * scale;
    final y0 = cmd.y0 * scale;
    final x1 = cmd.x1 * scale;
    final y1 = cmd.y1 * scale;
    final w = cmd.width * scale;

    if (cmd.kind == cmdLineVerticalThick) {
      // Vertical thick line: width is horizontal extent
      canvas.drawRect(
        Rect.fromLTRB(x0 - w / 2, y0, x0 + w / 2, y1),
        _fillPaint(),
      );
    } else {
      // Horizontal thick line: width is vertical extent
      canvas.drawRect(
        Rect.fromLTRB(x0, y0 - w / 2, x1, y0 + w / 2),
        _fillPaint(),
      );
    }
  }

  void _drawHDashedLine(Canvas canvas, RenderCommandDto cmd) {
    final paint = _strokePaint(cmd.width > 0 ? cmd.width : _lineWidth);
    final from = _pt(cmd.x0, cmd.y0);
    final to = _pt(cmd.x1, cmd.y0);
    final dashLen = cmd.dashLen * scale;
    if (dashLen < 0.01) {
      canvas.drawLine(from, to, paint);
      return;
    }
    final gapLen = dashLen; // gap = dash for now
    final totalLen = (to.dx - from.dx).abs();
    final cycle = dashLen + gapLen;
    var pos = 0.0;
    while (pos < totalLen) {
      final end = (pos + dashLen).clamp(0.0, totalLen);
      canvas.drawLine(
        Offset(from.dx + pos, from.dy),
        Offset(from.dx + end, from.dy),
        paint,
      );
      pos += cycle;
    }
  }

  void _drawVDashedLine(Canvas canvas, RenderCommandDto cmd) {
    final paint = _strokePaint(cmd.width > 0 ? cmd.width : _lineWidth);
    final x = cmd.x0 * scale;
    final y0 = cmd.y0 * scale;
    final y1 = cmd.y1 * scale;
    final dashLen = cmd.dashLen * scale;
    if (dashLen < 0.01) {
      canvas.drawLine(Offset(x, y0), Offset(x, y1), paint);
      return;
    }
    final gapLen = dashLen;
    final totalLen = (y1 - y0).abs();
    final cycle = dashLen + gapLen;
    var pos = 0.0;
    while (pos < totalLen) {
      final end = (pos + dashLen).clamp(0.0, totalLen);
      canvas.drawLine(Offset(x, y0 + pos), Offset(x, y0 + end), paint);
      pos += cycle;
    }
  }

  void _drawFrameRect(Canvas canvas, RenderCommandDto cmd) {
    final paint = _strokePaint(cmd.thickness > 0 ? cmd.thickness : _lineWidth);
    canvas.drawRect(
      Rect.fromLTWH(
        cmd.x0 * scale,
        cmd.y0 * scale,
        cmd.width * scale,
        cmd.height * scale,
      ),
      paint,
    );
  }

  void _drawStaffLine(Canvas canvas, RenderCommandDto cmd) {
    canvas.drawLine(
      _pt(cmd.x0, cmd.y0),
      _pt(cmd.x1, cmd.y0),
      _strokePaint(_staffLineWidth),
    );
  }

  void _drawStaff(Canvas canvas, RenderCommandDto cmd) {
    final paint = _strokePaint(_staffLineWidth);
    final x0 = cmd.x0 * scale;
    final x1 = cmd.x1 * scale;
    final y = cmd.y0 * scale;
    final nLines = cmd.nLines > 0 ? cmd.nLines : 5;
    final spacing = cmd.lineSpacing * scale;

    for (var i = 0; i < nLines; i++) {
      final ly = y + i * spacing;
      canvas.drawLine(Offset(x0, ly), Offset(x1, ly), paint);
    }
  }

  void _drawBarLine(Canvas canvas, RenderCommandDto cmd) {
    // barType: 0=single, 1=double, 2=finalDouble,
    //          3=repeatLeft, 4=repeatRight, 5=repeatBoth
    final x = cmd.x0 * scale;
    final top = cmd.y0 * scale;
    final bottom = cmd.y1 * scale;
    final thinPaint = _strokePaint(_barLineWidth);
    final thinW = _barLineWidth * scale;
    final thickW = thinW * 3;
    final gap = thinW * 2;

    switch (cmd.barType) {
      case 0: // Single
        canvas.drawLine(Offset(x, top), Offset(x, bottom), thinPaint);
        break;
      case 1: // Double (two thin lines)
        canvas.drawLine(Offset(x - gap, top), Offset(x - gap, bottom), thinPaint);
        canvas.drawLine(Offset(x, top), Offset(x, bottom), thinPaint);
        break;
      case 2: // Final double (thin + thick)
        canvas.drawLine(Offset(x - gap - thickW / 2, top),
            Offset(x - gap - thickW / 2, bottom), thinPaint);
        canvas.drawRect(
          Rect.fromLTRB(x - thickW / 2, top, x + thickW / 2, bottom),
          _fillPaint(),
        );
        break;
      case 3: // Repeat left (thick + thin + dots)
        canvas.drawRect(
          Rect.fromLTRB(x, top, x + thickW, bottom),
          _fillPaint(),
        );
        canvas.drawLine(Offset(x + thickW + gap, top),
            Offset(x + thickW + gap, bottom), thinPaint);
        _drawRepeatDotsAt(canvas, x + thickW + gap + gap, top, bottom);
        break;
      case 4: // Repeat right (dots + thin + thick)
        _drawRepeatDotsAt(canvas, x - gap - gap, top, bottom);
        canvas.drawLine(Offset(x - gap, top),
            Offset(x - gap, bottom), thinPaint);
        canvas.drawRect(
          Rect.fromLTRB(x - thickW / 2, top, x + thickW / 2, bottom),
          _fillPaint(),
        );
        break;
      case 5: // Repeat both
        _drawRepeatDotsAt(canvas, x - gap * 3, top, bottom);
        canvas.drawLine(Offset(x - gap, top),
            Offset(x - gap, bottom), thinPaint);
        canvas.drawRect(
          Rect.fromLTRB(x - thickW / 2, top, x + thickW / 2, bottom),
          _fillPaint(),
        );
        canvas.drawLine(Offset(x + gap, top),
            Offset(x + gap, bottom), thinPaint);
        _drawRepeatDotsAt(canvas, x + gap * 3, top, bottom);
        break;
      default:
        canvas.drawLine(Offset(x, top), Offset(x, bottom), thinPaint);
    }
  }

  /// Draw two repeat dots between top and bottom at x (in screen coords).
  void _drawRepeatDotsAt(Canvas canvas, double x, double top, double bottom) {
    final midY = (top + bottom) / 2;
    final staffSpacing = (bottom - top) / 4; // approximate for 5-line staff
    final dotRadius = staffSpacing * 0.2;
    final paint = _fillPaint();
    canvas.drawCircle(Offset(x, midY - staffSpacing * 0.5), dotRadius, paint);
    canvas.drawCircle(Offset(x, midY + staffSpacing * 0.5), dotRadius, paint);
  }

  void _drawConnectorLine(Canvas canvas, RenderCommandDto cmd) {
    canvas.drawLine(
      _pt(cmd.x0, cmd.y0),
      _pt(cmd.x0, cmd.y1),
      _strokePaint(_barLineWidth),
    );
  }

  void _drawLedgerLine(Canvas canvas, RenderCommandDto cmd) {
    final x = cmd.x0 * scale;
    final y = cmd.y0 * scale;
    final hw = cmd.width * scale;
    final paint = _strokePaint(_ledgerLineWidth);
    canvas.drawLine(Offset(x - hw, y), Offset(x + hw, y), paint);
  }

  void _drawRepeatDots(Canvas canvas, RenderCommandDto cmd) {
    // Two dots centered between top and bottom
    final x = cmd.x0 * scale;
    final top = cmd.y0 * scale;
    final bottom = cmd.y1 * scale;
    final midY = (top + bottom) / 2;
    final dotSpacing = (bottom - top) * 0.15;
    final dotRadius = 1.5 * scale;
    final paint = _fillPaint();

    canvas.drawCircle(Offset(x, midY - dotSpacing), dotRadius, paint);
    canvas.drawCircle(Offset(x, midY + dotSpacing), dotRadius, paint);
  }

  void _drawBeam(Canvas canvas, RenderCommandDto cmd) {
    // Beam: parallelogram from (x0,y0)-(x1,y1) with thickness
    final x0 = cmd.x0 * scale;
    final y0 = cmd.y0 * scale;
    final x1 = cmd.x1 * scale;
    final y1 = cmd.y1 * scale;
    final t = cmd.thickness * scale;

    // The beam direction (up/down) affects which side the thickness goes
    final dy0 = cmd.up0 ? -t : t;
    final dy1 = cmd.up1 ? -t : t;

    final path = ui.Path()
      ..moveTo(x0, y0)
      ..lineTo(x1, y1)
      ..lineTo(x1, y1 + dy1)
      ..lineTo(x0, y0 + dy0)
      ..close();
    canvas.drawPath(path, _fillPaint());
  }

  void _drawSlur(Canvas canvas, RenderCommandDto cmd) {
    final paint = _strokePaint(cmd.thickness > 0 ? cmd.thickness : _lineWidth);
    final path = ui.Path()
      ..moveTo(cmd.x0 * scale, cmd.y0 * scale)
      ..cubicTo(
        cmd.x1 * scale, cmd.y1 * scale,
        cmd.x2 * scale, cmd.y2 * scale,
        cmd.x3 * scale, cmd.y3 * scale,
      );
    canvas.drawPath(path, paint);
  }

  void _drawBracket(Canvas canvas, RenderCommandDto cmd) {
    // Bracket: vertical line with optional serifs at top and bottom
    final x = cmd.x0 * scale;
    final yTop = cmd.y0 * scale;
    final yBottom = cmd.y1 * scale;
    final paint = _strokePaint(_lineWidth);

    canvas.drawLine(Offset(x, yTop), Offset(x, yBottom), paint);
    // Small serifs
    final serifLen = 4.0 * scale;
    canvas.drawLine(Offset(x, yTop), Offset(x + serifLen, yTop), paint);
    canvas.drawLine(Offset(x, yBottom), Offset(x + serifLen, yBottom), paint);
  }

  void _drawBrace(Canvas canvas, RenderCommandDto cmd) {
    // Curly brace: approximate with Bezier curves
    final x = cmd.x0 * scale;
    final yTop = cmd.y0 * scale;
    final yBottom = cmd.y1 * scale;
    final midY = (yTop + yBottom) / 2;
    final braceWidth = 8.0 * scale; // visual width of the brace

    final paint = _strokePaint(1.5);
    final path = ui.Path()
      ..moveTo(x, yTop)
      ..cubicTo(
        x - braceWidth * 0.2, yTop + (midY - yTop) * 0.3,
        x - braceWidth, midY - (midY - yTop) * 0.3,
        x - braceWidth, midY,
      )
      ..cubicTo(
        x - braceWidth, midY + (yBottom - midY) * 0.3,
        x - braceWidth * 0.2, yBottom - (yBottom - midY) * 0.3,
        x, yBottom,
      );
    canvas.drawPath(path, paint);
  }

  void _drawNoteStem(Canvas canvas, RenderCommandDto cmd) {
    final paint = _strokePaint(cmd.width > 0 ? cmd.width : _stemLineWidth);
    canvas.drawLine(
      _pt(cmd.x0, cmd.y0),
      _pt(cmd.x0, cmd.y1),
      paint,
    );
  }

  void _drawMusicChar(Canvas canvas, RenderCommandDto cmd) {
    // SMuFL glyph: render as a Unicode character using the music font.
    final codePoint = cmd.glyphCode;
    final char = String.fromCharCode(codePoint);
    final fontSize = _musicFontSize(cmd.sizePercent);

    final style = ui.TextStyle(
      fontFamily: musicFontFamily,
      fontSize: fontSize,
      color: _color(),
    );

    final builder = ui.ParagraphBuilder(ui.ParagraphStyle(
      fontFamily: musicFontFamily,
      fontSize: fontSize,
    ))
      ..pushStyle(style)
      ..addText(char);

    final paragraph = builder.build()
      ..layout(const ui.ParagraphConstraints(width: double.infinity));

    // SMuFL baseline alignment
    final pos = _pt(cmd.x0, cmd.y0);
    final baselineOffset = paragraph.alphabeticBaseline;
    canvas.drawParagraph(paragraph, Offset(pos.dx, pos.dy - baselineOffset));
  }

  void _drawMusicString(Canvas canvas, RenderCommandDto cmd) {
    // Render multiple SMuFL glyphs as a string
    if (cmd.glyphCodes.isEmpty) return;
    final text = String.fromCharCodes(cmd.glyphCodes);
    final fontSize = _musicFontSize(cmd.sizePercent);

    final style = ui.TextStyle(
      fontFamily: musicFontFamily,
      fontSize: fontSize,
      color: _color(),
    );

    final builder = ui.ParagraphBuilder(ui.ParagraphStyle(
      fontFamily: musicFontFamily,
      fontSize: fontSize,
    ))
      ..pushStyle(style)
      ..addText(text);

    final paragraph = builder.build()
      ..layout(const ui.ParagraphConstraints(width: double.infinity));

    final pos = _pt(cmd.x0, cmd.y0);
    final baselineOffset = paragraph.alphabeticBaseline;
    canvas.drawParagraph(paragraph, Offset(pos.dx, pos.dy - baselineOffset));
  }

  void _drawTextString(Canvas canvas, RenderCommandDto cmd) {
    final fontSize = cmd.fontSize * scale;
    if (fontSize < 0.5) return;

    final style = ui.TextStyle(
      fontFamily: cmd.fontName.isNotEmpty ? cmd.fontName : textFontFamily,
      fontSize: fontSize,
      fontWeight: cmd.bold ? FontWeight.bold : FontWeight.normal,
      fontStyle: cmd.italic ? FontStyle.italic : FontStyle.normal,
      color: _color(),
    );

    final builder = ui.ParagraphBuilder(ui.ParagraphStyle(
      fontFamily: cmd.fontName.isNotEmpty ? cmd.fontName : textFontFamily,
      fontSize: fontSize,
    ))
      ..pushStyle(style)
      ..addText(cmd.text);

    final paragraph = builder.build()
      ..layout(const ui.ParagraphConstraints(width: double.infinity));

    final pos = _pt(cmd.x0, cmd.y0);
    final baselineOffset = paragraph.alphabeticBaseline;
    canvas.drawParagraph(paragraph, Offset(pos.dx, pos.dy - baselineOffset));
  }

  void _drawMusicColon(Canvas canvas, RenderCommandDto cmd) {
    // Music colon: two dots like a colon, used in repeat signs
    final x = cmd.x0 * scale;
    final y = cmd.y0 * scale;
    final ls = cmd.lineSpacing * scale;
    final dotRadius = ls * 0.2;
    final paint = _fillPaint();

    canvas.drawCircle(Offset(x, y - ls * 0.5), dotRadius, paint);
    canvas.drawCircle(Offset(x, y + ls * 0.5), dotRadius, paint);
  }

  /// Compute the font size for music glyphs from a sizePercent value.
  double _musicFontSize(double sizePercent) {
    // Base size: 24pt at 100%. Scale proportionally.
    final pct = sizePercent > 0 ? sizePercent : _musicSizePercent;
    return (24.0 * pct / 100.0) * scale;
  }
}

// ── Saved state for save/restore ──────────────────────────────────

class _SavedState {
  final double translateX, translateY;
  final double scaleX, scaleY;
  final double colorR, colorG, colorB, colorA;
  final double lineWidth;

  _SavedState({
    required this.translateX,
    required this.translateY,
    required this.scaleX,
    required this.scaleY,
    required this.colorR,
    required this.colorG,
    required this.colorB,
    required this.colorA,
    required this.lineWidth,
  });
}

// ── ScoreView widget ──────────────────────────────────────────────

/// Widget that displays a rendered score.
///
/// Extracts page dimensions from SetPageSize commands and stacks pages
/// vertically with scrolling, including gaps between pages.
class ScoreView extends StatelessWidget {
  final List<RenderCommandDto> commands;
  final double scale;
  final bool debug;

  const ScoreView({
    super.key,
    required this.commands,
    this.scale = 1.0,
    this.debug = false,
  });

  @override
  Widget build(BuildContext context) {
    // Extract page dimensions from SetPageSize commands.
    double pageWidth = 612; // default US Letter portrait
    double pageHeight = 792;
    int pageCount = 0;
    for (final cmd in commands) {
      if (cmd.kind == cmdSetPageSize) {
        if (cmd.width > 0) pageWidth = cmd.width;
        if (cmd.height > 0) pageHeight = cmd.height;
      }
      if (cmd.kind == cmdBeginPage) {
        pageCount++;
      }
    }
    if (pageCount == 0) pageCount = 1;

    // Account for page gaps and padding
    const padding = 24.0;
    final totalWidth = pageWidth * scale + padding * 2;
    final totalHeight =
        pageCount * (pageHeight * scale + ScorePainter.pageGap) + padding * 2;

    return Container(
      color: Colors.grey.shade200,
      child: SingleChildScrollView(
        scrollDirection: Axis.vertical,
        child: SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: Padding(
            padding: const EdgeInsets.all(padding),
            child: CustomPaint(
              size: Size(totalWidth - padding * 2, totalHeight - padding * 2),
              painter: ScorePainter(
                commands: commands,
                scale: scale,
                debug: debug,
              ),
            ),
          ),
        ),
      ),
    );
  }
}
