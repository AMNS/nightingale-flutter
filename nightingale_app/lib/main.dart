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
      ),
      home: const ScorePage(),
    );
  }
}

class ScorePage extends StatefulWidget {
  const ScorePage({super.key});

  @override
  State<ScorePage> createState() => _ScorePageState();
}

class _ScorePageState extends State<ScorePage> {
  List<RenderCommandDto>? _commands;
  String _status = 'Loading...';

  @override
  void initState() {
    super.initState();
    _loadScore();
  }

  Future<void> _loadScore() async {
    try {
      // Load a bundled NGL file from assets.
      final data = await rootBundle.load('assets/scores/01_me_and_lucy_simple.ngl');
      final bytes = data.buffer.asUint8List();
      final commands = await renderNglFromBytes(data: bytes);
      setState(() {
        _commands = commands;
        _status = '${commands.length} render commands';
      });
    } catch (e) {
      setState(() {
        _status = 'Error: $e';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Nightingale'),
        actions: [
          Center(
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: Text(_status, style: const TextStyle(fontSize: 12)),
            ),
          ),
        ],
      ),
      body: _commands == null
          ? const Center(child: CircularProgressIndicator())
          : ScoreView(
              commands: _commands!,
              scale: 1.5,
              debug: true,
            ),
    );
  }
}
