import 'package:flutter_test/flutter_test.dart';

import 'package:bandao_app/core/env/env.dart';

void main() {
  tearDown(Env.resetForTest);

  group('Env.compileTimeDefault', () {
    test('returns Android loopback when Platform.isAndroid', () {
      Env.debugSetIsAndroid(() => true);
      // No --dart-define is set in tests, so we never hit the dart-define
      // branch.
      expect(Env.compileTimeDefault(), 'http://10.0.2.2:9090');
    });

    test('returns localhost on non-Android (iOS / desktop)', () {
      Env.debugSetIsAndroid(() => false);
      expect(Env.compileTimeDefault(), 'http://localhost:9090');
    });
  });

  test('appId is the locked bundle id', () {
    expect(Env.appId, 'tw.ccmos.app.bandao');
  });
}
