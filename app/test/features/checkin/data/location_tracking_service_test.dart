import 'package:drift/native.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:geolocator/geolocator.dart';

import 'package:bandao_app/features/checkin/data/checkin_queue_db.dart';
import 'package:bandao_app/features/checkin/data/location_tracking_service.dart';

import '../../../helpers/fake_secure_storage.dart';

void main() {
  group('GeolocatorTrackingService throttle', () {
    late CheckinQueueDb db;
    late FakeSecureStorage storage;
    late GeolocatorTrackingService svc;

    setUp(() {
      db = CheckinQueueDb.forTesting(NativeDatabase.memory());
      storage = FakeSecureStorage();
      svc = GeolocatorTrackingService(db, storage);
      addTearDown(() async => db.close());
    });

    test('first ping is enqueued', () async {
      await svc.handlePositionForTest(_pos(25.0, 121.0), 'u1');
      final rows = await db.pickPendingLocationBatch(10);
      expect(rows, hasLength(1));
      expect(rows.first.lat, 25.0);
      expect(rows.first.lng, 121.0);
      expect(rows.first.appUserId, 'u1');
    });

    test('second ping within 60s is dropped', () async {
      // The throttle compares wall-clock now vs _lastEnqueuedAt. Two rapid
      // calls inside the same test will be < 60s apart by construction.
      await svc.handlePositionForTest(_pos(25.0, 121.0), 'u1');
      await svc.handlePositionForTest(_pos(25.001, 121.001), 'u1');
      await svc.handlePositionForTest(_pos(25.002, 121.002), 'u1');
      await svc.handlePositionForTest(_pos(25.003, 121.003), 'u1');
      await svc.handlePositionForTest(_pos(25.004, 121.004), 'u1');

      final rows = await db.pickPendingLocationBatch(10);
      expect(rows, hasLength(1), reason: '60s throttle drops the rest');
    });

    test('captures accuracy when finite', () async {
      await svc.handlePositionForTest(_pos(25.0, 121.0, accuracy: 12.5), 'u1');
      final rows = await db.pickPendingLocationBatch(10);
      expect(rows.first.accuracy, 12.5);
    });
  });
}

Position _pos(double lat, double lng, {double accuracy = 5.0}) {
  return Position(
    latitude: lat,
    longitude: lng,
    timestamp: DateTime.now(),
    accuracy: accuracy,
    altitude: 0,
    altitudeAccuracy: 0,
    heading: 0,
    headingAccuracy: 0,
    speed: 0,
    speedAccuracy: 0,
  );
}
