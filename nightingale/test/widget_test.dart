// Flutter unit tests for the Nightingale score renderer.
//
// These tests exercise the pure-Dart rendering layer (ScoreView / ScorePainter)
// using mock RenderCommandDto data constructed directly in Dart — no Rust
// bridge is loaded or initialized. They run headlessly in both CI environments:
//   flutter-linux:  flutter test (before flutter build linux)
//   flutter-macos:  flutter test (before flutter build macos)

import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nightingale/score_painter.dart';
import 'package:nightingale/src/rust/api/score.dart';

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Build a minimal RenderCommandDto with all required fields zeroed/empty,
/// overriding only the fields that matter for a given test.
RenderCommandDto cmd(int kind, {
  double x0 = 0, double y0 = 0,
  double x1 = 0, double y1 = 0,
  double x2 = 0, double y2 = 0,
  double x3 = 0, double y3 = 0,
  double width = 0, double height = 0,
  double thickness = 1,
  double lineSpacing = 8,
  double sizePercent = 100,
  double fontSize = 24,
  double dashLen = 0,
  double colorR = 0, double colorG = 0, double colorB = 0, double colorA = 1,
  int glyphCode = 0,
  int nLines = 5,
  int barType = 0,
  int pageNumber = 1,
  bool up0 = false, bool up1 = false,
  bool dashed = false, bool bold = false, bool italic = false,
  String text = '', String fontName = '',
}) {
  return RenderCommandDto(
    kind: kind,
    x0: x0, y0: y0, x1: x1, y1: y1,
    x2: x2, y2: y2, x3: x3, y3: y3,
    width: width, height: height,
    thickness: thickness, lineSpacing: lineSpacing,
    sizePercent: sizePercent, fontSize: fontSize,
    dashLen: dashLen,
    colorR: colorR, colorG: colorG, colorB: colorB, colorA: colorA,
    glyphCode: glyphCode, glyphCodes: Uint32List(0),
    nLines: nLines, barType: barType, pageNumber: pageNumber,
    up0: up0, up1: up1, dashed: dashed, bold: bold, italic: italic,
    text: text, fontName: fontName,
  );
}

/// A minimal one-page command stream: begin-page + one staff + end-page.
List<RenderCommandDto> minimalPageCommands({double lnSpace = 8}) => [
  cmd(cmdBeginPage, pageNumber: 1),
  cmd(cmdStaff, x0: 72, y0: 100, x1: 540, y1: 100, lineSpacing: lnSpace, nLines: 5),
  cmd(cmdEndPage),
];

// ── Tests ─────────────────────────────────────────────────────────────────────

void main() {
  // ── Command-kind constant sanity checks ────────────────────────────────────
  // Verify the Dart constants match the expected values from the Rust CMD_*
  // table. If Rust ever renumbers them the generated api/score.dart will change,
  // and this test will catch the mismatch before the painter silently breaks.
  group('CMD_* constants', () {
    test('line and bar primitives', () {
      expect(cmdLine,                  1);
      expect(cmdLineVerticalThick,     2);
      expect(cmdLineHorizontalThick,   3);
      expect(cmdHDashedLine,           4);
      expect(cmdVDashedLine,           5);
      expect(cmdFrameRect,             6);
      expect(cmdStaffLine,             7);
      expect(cmdStaff,                 8);
      expect(cmdBarLine,               9);
      expect(cmdConnectorLine,        10);
      expect(cmdLedgerLine,           11);
      expect(cmdRepeatDots,           12);
    });
    test('note / beam / slur / bracket', () {
      expect(cmdBeam,       13);
      expect(cmdSlur,       14);
      expect(cmdBracket,    15);
      expect(cmdBrace,      16);
      expect(cmdNoteStem,   17);
      expect(cmdMusicChar,  18);
    });
    test('page control', () {
      expect(cmdBeginPage,  26);
      expect(cmdEndPage,    27);
    });
  });

  // ── ScoreView widget tests ─────────────────────────────────────────────────
  group('ScoreView', () {
    testWidgets('renders without crashing with empty command list',
        (WidgetTester tester) async {
      await tester.pumpWidget(
        const MaterialApp(
          home: Scaffold(
            body: ScoreView(commands: [], scale: 1.0),
          ),
        ),
      );
      // Should not throw; ScoreView handles empty list gracefully.
      expect(find.byType(ScoreView), findsOneWidget);
    });

    testWidgets('renders without crashing with minimal one-page commands',
        (WidgetTester tester) async {
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: ScoreView(
              commands: minimalPageCommands(),
              scale: 1.0,
            ),
          ),
        ),
      );
      expect(find.byType(ScoreView), findsOneWidget);
    });

    testWidgets('accepts non-default scale factor',
        (WidgetTester tester) async {
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: ScoreView(
              commands: minimalPageCommands(),
              scale: 2.5,
            ),
          ),
        ),
      );
      expect(find.byType(ScoreView), findsOneWidget);
    });

    testWidgets('renders multi-page command stream',
        (WidgetTester tester) async {
      final commands = [
        ...minimalPageCommands(),
        cmd(cmdBeginPage, pageNumber: 2),
        cmd(cmdStaff, x0: 72, y0: 100, x1: 540, y1: 100, nLines: 5),
        cmd(cmdEndPage),
      ];
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: ScoreView(commands: commands, scale: 1.0),
          ),
        ),
      );
      expect(find.byType(ScoreView), findsOneWidget);
    });
  });

  // ── RenderCommandDto construction ─────────────────────────────────────────
  group('RenderCommandDto', () {
    test('const constructor produces correct field values', () {
      final c = cmd(cmdStaff,
        x0: 72, y0: 200, x1: 540, y1: 200,
        lineSpacing: 10, nLines: 5,
      );
      expect(c.kind,        cmdStaff);
      expect(c.x0,          72.0);
      expect(c.y0,          200.0);
      expect(c.x1,          540.0);
      expect(c.lineSpacing, 10.0);
      expect(c.nLines,      5);
      expect(c.glyphCodes.length, 0);
    });

    test('glyphCodes is a Uint32List', () {
      final c = cmd(cmdMusicChar, glyphCode: 0xE050);
      expect(c.glyphCodes, isA<Uint32List>());
      expect(c.glyphCode,  0xE050);
    });
  });
}
