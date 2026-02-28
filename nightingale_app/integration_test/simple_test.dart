import 'package:flutter_test/flutter_test.dart';
import 'package:nightingale_app/main.dart';
import 'package:nightingale_app/src/rust/frb_generated.dart';
import 'package:integration_test/integration_test.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async => await RustLib.init());
  testWidgets('App launches and shows Nightingale', (WidgetTester tester) async {
    await tester.pumpWidget(const NightingaleApp());
    expect(find.textContaining('Nightingale'), findsOneWidget);
  });
}
