// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'checkin_queue_db.dart';

// ignore_for_file: type=lint
class $PendingEventsTable extends PendingEvents
    with TableInfo<$PendingEventsTable, PendingEvent> {
  @override
  final GeneratedDatabase attachedDatabase;
  final String? _alias;
  $PendingEventsTable(this.attachedDatabase, [this._alias]);
  static const VerificationMeta _idMeta = const VerificationMeta('id');
  @override
  late final GeneratedColumn<int> id = GeneratedColumn<int>(
      'id', aliasedName, false,
      hasAutoIncrement: true,
      type: DriftSqlType.int,
      requiredDuringInsert: false,
      defaultConstraints:
          GeneratedColumn.constraintIsAlways('PRIMARY KEY AUTOINCREMENT'));
  static const VerificationMeta _appUserIdMeta =
      const VerificationMeta('appUserId');
  @override
  late final GeneratedColumn<String> appUserId = GeneratedColumn<String>(
      'app_user_id', aliasedName, false,
      type: DriftSqlType.string, requiredDuringInsert: true);
  static const VerificationMeta _eventTypeMeta =
      const VerificationMeta('eventType');
  @override
  late final GeneratedColumn<String> eventType = GeneratedColumn<String>(
      'event_type', aliasedName, false,
      type: DriftSqlType.string, requiredDuringInsert: true);
  static const VerificationMeta _latMeta = const VerificationMeta('lat');
  @override
  late final GeneratedColumn<double> lat = GeneratedColumn<double>(
      'lat', aliasedName, false,
      type: DriftSqlType.double, requiredDuringInsert: true);
  static const VerificationMeta _lngMeta = const VerificationMeta('lng');
  @override
  late final GeneratedColumn<double> lng = GeneratedColumn<double>(
      'lng', aliasedName, false,
      type: DriftSqlType.double, requiredDuringInsert: true);
  static const VerificationMeta _accuracyMeta =
      const VerificationMeta('accuracy');
  @override
  late final GeneratedColumn<double> accuracy = GeneratedColumn<double>(
      'accuracy', aliasedName, true,
      type: DriftSqlType.double, requiredDuringInsert: false);
  static const VerificationMeta _manualLabelMeta =
      const VerificationMeta('manualLabel');
  @override
  late final GeneratedColumn<String> manualLabel = GeneratedColumn<String>(
      'manual_label', aliasedName, true,
      type: DriftSqlType.string, requiredDuringInsert: false);
  static const VerificationMeta _occurredAtClientMeta =
      const VerificationMeta('occurredAtClient');
  @override
  late final GeneratedColumn<String> occurredAtClient = GeneratedColumn<String>(
      'occurred_at_client', aliasedName, false,
      type: DriftSqlType.string, requiredDuringInsert: true);
  static const VerificationMeta _statusMeta = const VerificationMeta('status');
  @override
  late final GeneratedColumn<String> status = GeneratedColumn<String>(
      'status', aliasedName, false,
      type: DriftSqlType.string,
      requiredDuringInsert: false,
      defaultValue: const Constant('pending'));
  static const VerificationMeta _attemptsMeta =
      const VerificationMeta('attempts');
  @override
  late final GeneratedColumn<int> attempts = GeneratedColumn<int>(
      'attempts', aliasedName, false,
      type: DriftSqlType.int,
      requiredDuringInsert: false,
      defaultValue: const Constant(0));
  static const VerificationMeta _lastErrorCodeMeta =
      const VerificationMeta('lastErrorCode');
  @override
  late final GeneratedColumn<String> lastErrorCode = GeneratedColumn<String>(
      'last_error_code', aliasedName, true,
      type: DriftSqlType.string, requiredDuringInsert: false);
  static const VerificationMeta _lastErrorMessageMeta =
      const VerificationMeta('lastErrorMessage');
  @override
  late final GeneratedColumn<String> lastErrorMessage = GeneratedColumn<String>(
      'last_error_message', aliasedName, true,
      type: DriftSqlType.string, requiredDuringInsert: false);
  static const VerificationMeta _lastAttemptAtMeta =
      const VerificationMeta('lastAttemptAt');
  @override
  late final GeneratedColumn<String> lastAttemptAt = GeneratedColumn<String>(
      'last_attempt_at', aliasedName, true,
      type: DriftSqlType.string, requiredDuringInsert: false);
  static const VerificationMeta _enqueuedAtMeta =
      const VerificationMeta('enqueuedAt');
  @override
  late final GeneratedColumn<String> enqueuedAt = GeneratedColumn<String>(
      'enqueued_at', aliasedName, false,
      type: DriftSqlType.string, requiredDuringInsert: true);
  @override
  List<GeneratedColumn> get $columns => [
        id,
        appUserId,
        eventType,
        lat,
        lng,
        accuracy,
        manualLabel,
        occurredAtClient,
        status,
        attempts,
        lastErrorCode,
        lastErrorMessage,
        lastAttemptAt,
        enqueuedAt
      ];
  @override
  String get aliasedName => _alias ?? actualTableName;
  @override
  String get actualTableName => $name;
  static const String $name = 'pending_events';
  @override
  VerificationContext validateIntegrity(Insertable<PendingEvent> instance,
      {bool isInserting = false}) {
    final context = VerificationContext();
    final data = instance.toColumns(true);
    if (data.containsKey('id')) {
      context.handle(_idMeta, id.isAcceptableOrUnknown(data['id']!, _idMeta));
    }
    if (data.containsKey('app_user_id')) {
      context.handle(
          _appUserIdMeta,
          appUserId.isAcceptableOrUnknown(
              data['app_user_id']!, _appUserIdMeta));
    } else if (isInserting) {
      context.missing(_appUserIdMeta);
    }
    if (data.containsKey('event_type')) {
      context.handle(_eventTypeMeta,
          eventType.isAcceptableOrUnknown(data['event_type']!, _eventTypeMeta));
    } else if (isInserting) {
      context.missing(_eventTypeMeta);
    }
    if (data.containsKey('lat')) {
      context.handle(
          _latMeta, lat.isAcceptableOrUnknown(data['lat']!, _latMeta));
    } else if (isInserting) {
      context.missing(_latMeta);
    }
    if (data.containsKey('lng')) {
      context.handle(
          _lngMeta, lng.isAcceptableOrUnknown(data['lng']!, _lngMeta));
    } else if (isInserting) {
      context.missing(_lngMeta);
    }
    if (data.containsKey('accuracy')) {
      context.handle(_accuracyMeta,
          accuracy.isAcceptableOrUnknown(data['accuracy']!, _accuracyMeta));
    }
    if (data.containsKey('manual_label')) {
      context.handle(
          _manualLabelMeta,
          manualLabel.isAcceptableOrUnknown(
              data['manual_label']!, _manualLabelMeta));
    }
    if (data.containsKey('occurred_at_client')) {
      context.handle(
          _occurredAtClientMeta,
          occurredAtClient.isAcceptableOrUnknown(
              data['occurred_at_client']!, _occurredAtClientMeta));
    } else if (isInserting) {
      context.missing(_occurredAtClientMeta);
    }
    if (data.containsKey('status')) {
      context.handle(_statusMeta,
          status.isAcceptableOrUnknown(data['status']!, _statusMeta));
    }
    if (data.containsKey('attempts')) {
      context.handle(_attemptsMeta,
          attempts.isAcceptableOrUnknown(data['attempts']!, _attemptsMeta));
    }
    if (data.containsKey('last_error_code')) {
      context.handle(
          _lastErrorCodeMeta,
          lastErrorCode.isAcceptableOrUnknown(
              data['last_error_code']!, _lastErrorCodeMeta));
    }
    if (data.containsKey('last_error_message')) {
      context.handle(
          _lastErrorMessageMeta,
          lastErrorMessage.isAcceptableOrUnknown(
              data['last_error_message']!, _lastErrorMessageMeta));
    }
    if (data.containsKey('last_attempt_at')) {
      context.handle(
          _lastAttemptAtMeta,
          lastAttemptAt.isAcceptableOrUnknown(
              data['last_attempt_at']!, _lastAttemptAtMeta));
    }
    if (data.containsKey('enqueued_at')) {
      context.handle(
          _enqueuedAtMeta,
          enqueuedAt.isAcceptableOrUnknown(
              data['enqueued_at']!, _enqueuedAtMeta));
    } else if (isInserting) {
      context.missing(_enqueuedAtMeta);
    }
    return context;
  }

  @override
  Set<GeneratedColumn> get $primaryKey => {id};
  @override
  PendingEvent map(Map<String, dynamic> data, {String? tablePrefix}) {
    final effectivePrefix = tablePrefix != null ? '$tablePrefix.' : '';
    return PendingEvent(
      id: attachedDatabase.typeMapping
          .read(DriftSqlType.int, data['${effectivePrefix}id'])!,
      appUserId: attachedDatabase.typeMapping
          .read(DriftSqlType.string, data['${effectivePrefix}app_user_id'])!,
      eventType: attachedDatabase.typeMapping
          .read(DriftSqlType.string, data['${effectivePrefix}event_type'])!,
      lat: attachedDatabase.typeMapping
          .read(DriftSqlType.double, data['${effectivePrefix}lat'])!,
      lng: attachedDatabase.typeMapping
          .read(DriftSqlType.double, data['${effectivePrefix}lng'])!,
      accuracy: attachedDatabase.typeMapping
          .read(DriftSqlType.double, data['${effectivePrefix}accuracy']),
      manualLabel: attachedDatabase.typeMapping
          .read(DriftSqlType.string, data['${effectivePrefix}manual_label']),
      occurredAtClient: attachedDatabase.typeMapping.read(
          DriftSqlType.string, data['${effectivePrefix}occurred_at_client'])!,
      status: attachedDatabase.typeMapping
          .read(DriftSqlType.string, data['${effectivePrefix}status'])!,
      attempts: attachedDatabase.typeMapping
          .read(DriftSqlType.int, data['${effectivePrefix}attempts'])!,
      lastErrorCode: attachedDatabase.typeMapping
          .read(DriftSqlType.string, data['${effectivePrefix}last_error_code']),
      lastErrorMessage: attachedDatabase.typeMapping.read(
          DriftSqlType.string, data['${effectivePrefix}last_error_message']),
      lastAttemptAt: attachedDatabase.typeMapping
          .read(DriftSqlType.string, data['${effectivePrefix}last_attempt_at']),
      enqueuedAt: attachedDatabase.typeMapping
          .read(DriftSqlType.string, data['${effectivePrefix}enqueued_at'])!,
    );
  }

  @override
  $PendingEventsTable createAlias(String alias) {
    return $PendingEventsTable(attachedDatabase, alias);
  }
}

class PendingEvent extends DataClass implements Insertable<PendingEvent> {
  final int id;
  final String appUserId;
  final String eventType;
  final double lat;
  final double lng;
  final double? accuracy;
  final String? manualLabel;
  final String occurredAtClient;
  final String status;
  final int attempts;
  final String? lastErrorCode;
  final String? lastErrorMessage;
  final String? lastAttemptAt;
  final String enqueuedAt;
  const PendingEvent(
      {required this.id,
      required this.appUserId,
      required this.eventType,
      required this.lat,
      required this.lng,
      this.accuracy,
      this.manualLabel,
      required this.occurredAtClient,
      required this.status,
      required this.attempts,
      this.lastErrorCode,
      this.lastErrorMessage,
      this.lastAttemptAt,
      required this.enqueuedAt});
  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    map['id'] = Variable<int>(id);
    map['app_user_id'] = Variable<String>(appUserId);
    map['event_type'] = Variable<String>(eventType);
    map['lat'] = Variable<double>(lat);
    map['lng'] = Variable<double>(lng);
    if (!nullToAbsent || accuracy != null) {
      map['accuracy'] = Variable<double>(accuracy);
    }
    if (!nullToAbsent || manualLabel != null) {
      map['manual_label'] = Variable<String>(manualLabel);
    }
    map['occurred_at_client'] = Variable<String>(occurredAtClient);
    map['status'] = Variable<String>(status);
    map['attempts'] = Variable<int>(attempts);
    if (!nullToAbsent || lastErrorCode != null) {
      map['last_error_code'] = Variable<String>(lastErrorCode);
    }
    if (!nullToAbsent || lastErrorMessage != null) {
      map['last_error_message'] = Variable<String>(lastErrorMessage);
    }
    if (!nullToAbsent || lastAttemptAt != null) {
      map['last_attempt_at'] = Variable<String>(lastAttemptAt);
    }
    map['enqueued_at'] = Variable<String>(enqueuedAt);
    return map;
  }

  PendingEventsCompanion toCompanion(bool nullToAbsent) {
    return PendingEventsCompanion(
      id: Value(id),
      appUserId: Value(appUserId),
      eventType: Value(eventType),
      lat: Value(lat),
      lng: Value(lng),
      accuracy: accuracy == null && nullToAbsent
          ? const Value.absent()
          : Value(accuracy),
      manualLabel: manualLabel == null && nullToAbsent
          ? const Value.absent()
          : Value(manualLabel),
      occurredAtClient: Value(occurredAtClient),
      status: Value(status),
      attempts: Value(attempts),
      lastErrorCode: lastErrorCode == null && nullToAbsent
          ? const Value.absent()
          : Value(lastErrorCode),
      lastErrorMessage: lastErrorMessage == null && nullToAbsent
          ? const Value.absent()
          : Value(lastErrorMessage),
      lastAttemptAt: lastAttemptAt == null && nullToAbsent
          ? const Value.absent()
          : Value(lastAttemptAt),
      enqueuedAt: Value(enqueuedAt),
    );
  }

  factory PendingEvent.fromJson(Map<String, dynamic> json,
      {ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return PendingEvent(
      id: serializer.fromJson<int>(json['id']),
      appUserId: serializer.fromJson<String>(json['appUserId']),
      eventType: serializer.fromJson<String>(json['eventType']),
      lat: serializer.fromJson<double>(json['lat']),
      lng: serializer.fromJson<double>(json['lng']),
      accuracy: serializer.fromJson<double?>(json['accuracy']),
      manualLabel: serializer.fromJson<String?>(json['manualLabel']),
      occurredAtClient: serializer.fromJson<String>(json['occurredAtClient']),
      status: serializer.fromJson<String>(json['status']),
      attempts: serializer.fromJson<int>(json['attempts']),
      lastErrorCode: serializer.fromJson<String?>(json['lastErrorCode']),
      lastErrorMessage: serializer.fromJson<String?>(json['lastErrorMessage']),
      lastAttemptAt: serializer.fromJson<String?>(json['lastAttemptAt']),
      enqueuedAt: serializer.fromJson<String>(json['enqueuedAt']),
    );
  }
  @override
  Map<String, dynamic> toJson({ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return <String, dynamic>{
      'id': serializer.toJson<int>(id),
      'appUserId': serializer.toJson<String>(appUserId),
      'eventType': serializer.toJson<String>(eventType),
      'lat': serializer.toJson<double>(lat),
      'lng': serializer.toJson<double>(lng),
      'accuracy': serializer.toJson<double?>(accuracy),
      'manualLabel': serializer.toJson<String?>(manualLabel),
      'occurredAtClient': serializer.toJson<String>(occurredAtClient),
      'status': serializer.toJson<String>(status),
      'attempts': serializer.toJson<int>(attempts),
      'lastErrorCode': serializer.toJson<String?>(lastErrorCode),
      'lastErrorMessage': serializer.toJson<String?>(lastErrorMessage),
      'lastAttemptAt': serializer.toJson<String?>(lastAttemptAt),
      'enqueuedAt': serializer.toJson<String>(enqueuedAt),
    };
  }

  PendingEvent copyWith(
          {int? id,
          String? appUserId,
          String? eventType,
          double? lat,
          double? lng,
          Value<double?> accuracy = const Value.absent(),
          Value<String?> manualLabel = const Value.absent(),
          String? occurredAtClient,
          String? status,
          int? attempts,
          Value<String?> lastErrorCode = const Value.absent(),
          Value<String?> lastErrorMessage = const Value.absent(),
          Value<String?> lastAttemptAt = const Value.absent(),
          String? enqueuedAt}) =>
      PendingEvent(
        id: id ?? this.id,
        appUserId: appUserId ?? this.appUserId,
        eventType: eventType ?? this.eventType,
        lat: lat ?? this.lat,
        lng: lng ?? this.lng,
        accuracy: accuracy.present ? accuracy.value : this.accuracy,
        manualLabel: manualLabel.present ? manualLabel.value : this.manualLabel,
        occurredAtClient: occurredAtClient ?? this.occurredAtClient,
        status: status ?? this.status,
        attempts: attempts ?? this.attempts,
        lastErrorCode:
            lastErrorCode.present ? lastErrorCode.value : this.lastErrorCode,
        lastErrorMessage: lastErrorMessage.present
            ? lastErrorMessage.value
            : this.lastErrorMessage,
        lastAttemptAt:
            lastAttemptAt.present ? lastAttemptAt.value : this.lastAttemptAt,
        enqueuedAt: enqueuedAt ?? this.enqueuedAt,
      );
  PendingEvent copyWithCompanion(PendingEventsCompanion data) {
    return PendingEvent(
      id: data.id.present ? data.id.value : this.id,
      appUserId: data.appUserId.present ? data.appUserId.value : this.appUserId,
      eventType: data.eventType.present ? data.eventType.value : this.eventType,
      lat: data.lat.present ? data.lat.value : this.lat,
      lng: data.lng.present ? data.lng.value : this.lng,
      accuracy: data.accuracy.present ? data.accuracy.value : this.accuracy,
      manualLabel:
          data.manualLabel.present ? data.manualLabel.value : this.manualLabel,
      occurredAtClient: data.occurredAtClient.present
          ? data.occurredAtClient.value
          : this.occurredAtClient,
      status: data.status.present ? data.status.value : this.status,
      attempts: data.attempts.present ? data.attempts.value : this.attempts,
      lastErrorCode: data.lastErrorCode.present
          ? data.lastErrorCode.value
          : this.lastErrorCode,
      lastErrorMessage: data.lastErrorMessage.present
          ? data.lastErrorMessage.value
          : this.lastErrorMessage,
      lastAttemptAt: data.lastAttemptAt.present
          ? data.lastAttemptAt.value
          : this.lastAttemptAt,
      enqueuedAt:
          data.enqueuedAt.present ? data.enqueuedAt.value : this.enqueuedAt,
    );
  }

  @override
  String toString() {
    return (StringBuffer('PendingEvent(')
          ..write('id: $id, ')
          ..write('appUserId: $appUserId, ')
          ..write('eventType: $eventType, ')
          ..write('lat: $lat, ')
          ..write('lng: $lng, ')
          ..write('accuracy: $accuracy, ')
          ..write('manualLabel: $manualLabel, ')
          ..write('occurredAtClient: $occurredAtClient, ')
          ..write('status: $status, ')
          ..write('attempts: $attempts, ')
          ..write('lastErrorCode: $lastErrorCode, ')
          ..write('lastErrorMessage: $lastErrorMessage, ')
          ..write('lastAttemptAt: $lastAttemptAt, ')
          ..write('enqueuedAt: $enqueuedAt')
          ..write(')'))
        .toString();
  }

  @override
  int get hashCode => Object.hash(
      id,
      appUserId,
      eventType,
      lat,
      lng,
      accuracy,
      manualLabel,
      occurredAtClient,
      status,
      attempts,
      lastErrorCode,
      lastErrorMessage,
      lastAttemptAt,
      enqueuedAt);
  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is PendingEvent &&
          other.id == this.id &&
          other.appUserId == this.appUserId &&
          other.eventType == this.eventType &&
          other.lat == this.lat &&
          other.lng == this.lng &&
          other.accuracy == this.accuracy &&
          other.manualLabel == this.manualLabel &&
          other.occurredAtClient == this.occurredAtClient &&
          other.status == this.status &&
          other.attempts == this.attempts &&
          other.lastErrorCode == this.lastErrorCode &&
          other.lastErrorMessage == this.lastErrorMessage &&
          other.lastAttemptAt == this.lastAttemptAt &&
          other.enqueuedAt == this.enqueuedAt);
}

class PendingEventsCompanion extends UpdateCompanion<PendingEvent> {
  final Value<int> id;
  final Value<String> appUserId;
  final Value<String> eventType;
  final Value<double> lat;
  final Value<double> lng;
  final Value<double?> accuracy;
  final Value<String?> manualLabel;
  final Value<String> occurredAtClient;
  final Value<String> status;
  final Value<int> attempts;
  final Value<String?> lastErrorCode;
  final Value<String?> lastErrorMessage;
  final Value<String?> lastAttemptAt;
  final Value<String> enqueuedAt;
  const PendingEventsCompanion({
    this.id = const Value.absent(),
    this.appUserId = const Value.absent(),
    this.eventType = const Value.absent(),
    this.lat = const Value.absent(),
    this.lng = const Value.absent(),
    this.accuracy = const Value.absent(),
    this.manualLabel = const Value.absent(),
    this.occurredAtClient = const Value.absent(),
    this.status = const Value.absent(),
    this.attempts = const Value.absent(),
    this.lastErrorCode = const Value.absent(),
    this.lastErrorMessage = const Value.absent(),
    this.lastAttemptAt = const Value.absent(),
    this.enqueuedAt = const Value.absent(),
  });
  PendingEventsCompanion.insert({
    this.id = const Value.absent(),
    required String appUserId,
    required String eventType,
    required double lat,
    required double lng,
    this.accuracy = const Value.absent(),
    this.manualLabel = const Value.absent(),
    required String occurredAtClient,
    this.status = const Value.absent(),
    this.attempts = const Value.absent(),
    this.lastErrorCode = const Value.absent(),
    this.lastErrorMessage = const Value.absent(),
    this.lastAttemptAt = const Value.absent(),
    required String enqueuedAt,
  })  : appUserId = Value(appUserId),
        eventType = Value(eventType),
        lat = Value(lat),
        lng = Value(lng),
        occurredAtClient = Value(occurredAtClient),
        enqueuedAt = Value(enqueuedAt);
  static Insertable<PendingEvent> custom({
    Expression<int>? id,
    Expression<String>? appUserId,
    Expression<String>? eventType,
    Expression<double>? lat,
    Expression<double>? lng,
    Expression<double>? accuracy,
    Expression<String>? manualLabel,
    Expression<String>? occurredAtClient,
    Expression<String>? status,
    Expression<int>? attempts,
    Expression<String>? lastErrorCode,
    Expression<String>? lastErrorMessage,
    Expression<String>? lastAttemptAt,
    Expression<String>? enqueuedAt,
  }) {
    return RawValuesInsertable({
      if (id != null) 'id': id,
      if (appUserId != null) 'app_user_id': appUserId,
      if (eventType != null) 'event_type': eventType,
      if (lat != null) 'lat': lat,
      if (lng != null) 'lng': lng,
      if (accuracy != null) 'accuracy': accuracy,
      if (manualLabel != null) 'manual_label': manualLabel,
      if (occurredAtClient != null) 'occurred_at_client': occurredAtClient,
      if (status != null) 'status': status,
      if (attempts != null) 'attempts': attempts,
      if (lastErrorCode != null) 'last_error_code': lastErrorCode,
      if (lastErrorMessage != null) 'last_error_message': lastErrorMessage,
      if (lastAttemptAt != null) 'last_attempt_at': lastAttemptAt,
      if (enqueuedAt != null) 'enqueued_at': enqueuedAt,
    });
  }

  PendingEventsCompanion copyWith(
      {Value<int>? id,
      Value<String>? appUserId,
      Value<String>? eventType,
      Value<double>? lat,
      Value<double>? lng,
      Value<double?>? accuracy,
      Value<String?>? manualLabel,
      Value<String>? occurredAtClient,
      Value<String>? status,
      Value<int>? attempts,
      Value<String?>? lastErrorCode,
      Value<String?>? lastErrorMessage,
      Value<String?>? lastAttemptAt,
      Value<String>? enqueuedAt}) {
    return PendingEventsCompanion(
      id: id ?? this.id,
      appUserId: appUserId ?? this.appUserId,
      eventType: eventType ?? this.eventType,
      lat: lat ?? this.lat,
      lng: lng ?? this.lng,
      accuracy: accuracy ?? this.accuracy,
      manualLabel: manualLabel ?? this.manualLabel,
      occurredAtClient: occurredAtClient ?? this.occurredAtClient,
      status: status ?? this.status,
      attempts: attempts ?? this.attempts,
      lastErrorCode: lastErrorCode ?? this.lastErrorCode,
      lastErrorMessage: lastErrorMessage ?? this.lastErrorMessage,
      lastAttemptAt: lastAttemptAt ?? this.lastAttemptAt,
      enqueuedAt: enqueuedAt ?? this.enqueuedAt,
    );
  }

  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    if (id.present) {
      map['id'] = Variable<int>(id.value);
    }
    if (appUserId.present) {
      map['app_user_id'] = Variable<String>(appUserId.value);
    }
    if (eventType.present) {
      map['event_type'] = Variable<String>(eventType.value);
    }
    if (lat.present) {
      map['lat'] = Variable<double>(lat.value);
    }
    if (lng.present) {
      map['lng'] = Variable<double>(lng.value);
    }
    if (accuracy.present) {
      map['accuracy'] = Variable<double>(accuracy.value);
    }
    if (manualLabel.present) {
      map['manual_label'] = Variable<String>(manualLabel.value);
    }
    if (occurredAtClient.present) {
      map['occurred_at_client'] = Variable<String>(occurredAtClient.value);
    }
    if (status.present) {
      map['status'] = Variable<String>(status.value);
    }
    if (attempts.present) {
      map['attempts'] = Variable<int>(attempts.value);
    }
    if (lastErrorCode.present) {
      map['last_error_code'] = Variable<String>(lastErrorCode.value);
    }
    if (lastErrorMessage.present) {
      map['last_error_message'] = Variable<String>(lastErrorMessage.value);
    }
    if (lastAttemptAt.present) {
      map['last_attempt_at'] = Variable<String>(lastAttemptAt.value);
    }
    if (enqueuedAt.present) {
      map['enqueued_at'] = Variable<String>(enqueuedAt.value);
    }
    return map;
  }

  @override
  String toString() {
    return (StringBuffer('PendingEventsCompanion(')
          ..write('id: $id, ')
          ..write('appUserId: $appUserId, ')
          ..write('eventType: $eventType, ')
          ..write('lat: $lat, ')
          ..write('lng: $lng, ')
          ..write('accuracy: $accuracy, ')
          ..write('manualLabel: $manualLabel, ')
          ..write('occurredAtClient: $occurredAtClient, ')
          ..write('status: $status, ')
          ..write('attempts: $attempts, ')
          ..write('lastErrorCode: $lastErrorCode, ')
          ..write('lastErrorMessage: $lastErrorMessage, ')
          ..write('lastAttemptAt: $lastAttemptAt, ')
          ..write('enqueuedAt: $enqueuedAt')
          ..write(')'))
        .toString();
  }
}

class $PendingLocationPingsTable extends PendingLocationPings
    with TableInfo<$PendingLocationPingsTable, PendingLocationPing> {
  @override
  final GeneratedDatabase attachedDatabase;
  final String? _alias;
  $PendingLocationPingsTable(this.attachedDatabase, [this._alias]);
  static const VerificationMeta _idMeta = const VerificationMeta('id');
  @override
  late final GeneratedColumn<int> id = GeneratedColumn<int>(
      'id', aliasedName, false,
      hasAutoIncrement: true,
      type: DriftSqlType.int,
      requiredDuringInsert: false,
      defaultConstraints:
          GeneratedColumn.constraintIsAlways('PRIMARY KEY AUTOINCREMENT'));
  static const VerificationMeta _appUserIdMeta =
      const VerificationMeta('appUserId');
  @override
  late final GeneratedColumn<String> appUserId = GeneratedColumn<String>(
      'app_user_id', aliasedName, false,
      type: DriftSqlType.string, requiredDuringInsert: true);
  static const VerificationMeta _latMeta = const VerificationMeta('lat');
  @override
  late final GeneratedColumn<double> lat = GeneratedColumn<double>(
      'lat', aliasedName, false,
      type: DriftSqlType.double, requiredDuringInsert: true);
  static const VerificationMeta _lngMeta = const VerificationMeta('lng');
  @override
  late final GeneratedColumn<double> lng = GeneratedColumn<double>(
      'lng', aliasedName, false,
      type: DriftSqlType.double, requiredDuringInsert: true);
  static const VerificationMeta _accuracyMeta =
      const VerificationMeta('accuracy');
  @override
  late final GeneratedColumn<double> accuracy = GeneratedColumn<double>(
      'accuracy', aliasedName, true,
      type: DriftSqlType.double, requiredDuringInsert: false);
  static const VerificationMeta _occurredAtClientMeta =
      const VerificationMeta('occurredAtClient');
  @override
  late final GeneratedColumn<String> occurredAtClient = GeneratedColumn<String>(
      'occurred_at_client', aliasedName, false,
      type: DriftSqlType.string, requiredDuringInsert: true);
  static const VerificationMeta _statusMeta = const VerificationMeta('status');
  @override
  late final GeneratedColumn<String> status = GeneratedColumn<String>(
      'status', aliasedName, false,
      type: DriftSqlType.string,
      requiredDuringInsert: false,
      defaultValue: const Constant('pending'));
  static const VerificationMeta _attemptsMeta =
      const VerificationMeta('attempts');
  @override
  late final GeneratedColumn<int> attempts = GeneratedColumn<int>(
      'attempts', aliasedName, false,
      type: DriftSqlType.int,
      requiredDuringInsert: false,
      defaultValue: const Constant(0));
  static const VerificationMeta _lastErrorCodeMeta =
      const VerificationMeta('lastErrorCode');
  @override
  late final GeneratedColumn<String> lastErrorCode = GeneratedColumn<String>(
      'last_error_code', aliasedName, true,
      type: DriftSqlType.string, requiredDuringInsert: false);
  static const VerificationMeta _lastErrorMessageMeta =
      const VerificationMeta('lastErrorMessage');
  @override
  late final GeneratedColumn<String> lastErrorMessage = GeneratedColumn<String>(
      'last_error_message', aliasedName, true,
      type: DriftSqlType.string, requiredDuringInsert: false);
  static const VerificationMeta _lastAttemptAtMeta =
      const VerificationMeta('lastAttemptAt');
  @override
  late final GeneratedColumn<String> lastAttemptAt = GeneratedColumn<String>(
      'last_attempt_at', aliasedName, true,
      type: DriftSqlType.string, requiredDuringInsert: false);
  static const VerificationMeta _enqueuedAtMeta =
      const VerificationMeta('enqueuedAt');
  @override
  late final GeneratedColumn<String> enqueuedAt = GeneratedColumn<String>(
      'enqueued_at', aliasedName, false,
      type: DriftSqlType.string, requiredDuringInsert: true);
  @override
  List<GeneratedColumn> get $columns => [
        id,
        appUserId,
        lat,
        lng,
        accuracy,
        occurredAtClient,
        status,
        attempts,
        lastErrorCode,
        lastErrorMessage,
        lastAttemptAt,
        enqueuedAt
      ];
  @override
  String get aliasedName => _alias ?? actualTableName;
  @override
  String get actualTableName => $name;
  static const String $name = 'pending_location_pings';
  @override
  VerificationContext validateIntegrity(
      Insertable<PendingLocationPing> instance,
      {bool isInserting = false}) {
    final context = VerificationContext();
    final data = instance.toColumns(true);
    if (data.containsKey('id')) {
      context.handle(_idMeta, id.isAcceptableOrUnknown(data['id']!, _idMeta));
    }
    if (data.containsKey('app_user_id')) {
      context.handle(
          _appUserIdMeta,
          appUserId.isAcceptableOrUnknown(
              data['app_user_id']!, _appUserIdMeta));
    } else if (isInserting) {
      context.missing(_appUserIdMeta);
    }
    if (data.containsKey('lat')) {
      context.handle(
          _latMeta, lat.isAcceptableOrUnknown(data['lat']!, _latMeta));
    } else if (isInserting) {
      context.missing(_latMeta);
    }
    if (data.containsKey('lng')) {
      context.handle(
          _lngMeta, lng.isAcceptableOrUnknown(data['lng']!, _lngMeta));
    } else if (isInserting) {
      context.missing(_lngMeta);
    }
    if (data.containsKey('accuracy')) {
      context.handle(_accuracyMeta,
          accuracy.isAcceptableOrUnknown(data['accuracy']!, _accuracyMeta));
    }
    if (data.containsKey('occurred_at_client')) {
      context.handle(
          _occurredAtClientMeta,
          occurredAtClient.isAcceptableOrUnknown(
              data['occurred_at_client']!, _occurredAtClientMeta));
    } else if (isInserting) {
      context.missing(_occurredAtClientMeta);
    }
    if (data.containsKey('status')) {
      context.handle(_statusMeta,
          status.isAcceptableOrUnknown(data['status']!, _statusMeta));
    }
    if (data.containsKey('attempts')) {
      context.handle(_attemptsMeta,
          attempts.isAcceptableOrUnknown(data['attempts']!, _attemptsMeta));
    }
    if (data.containsKey('last_error_code')) {
      context.handle(
          _lastErrorCodeMeta,
          lastErrorCode.isAcceptableOrUnknown(
              data['last_error_code']!, _lastErrorCodeMeta));
    }
    if (data.containsKey('last_error_message')) {
      context.handle(
          _lastErrorMessageMeta,
          lastErrorMessage.isAcceptableOrUnknown(
              data['last_error_message']!, _lastErrorMessageMeta));
    }
    if (data.containsKey('last_attempt_at')) {
      context.handle(
          _lastAttemptAtMeta,
          lastAttemptAt.isAcceptableOrUnknown(
              data['last_attempt_at']!, _lastAttemptAtMeta));
    }
    if (data.containsKey('enqueued_at')) {
      context.handle(
          _enqueuedAtMeta,
          enqueuedAt.isAcceptableOrUnknown(
              data['enqueued_at']!, _enqueuedAtMeta));
    } else if (isInserting) {
      context.missing(_enqueuedAtMeta);
    }
    return context;
  }

  @override
  Set<GeneratedColumn> get $primaryKey => {id};
  @override
  PendingLocationPing map(Map<String, dynamic> data, {String? tablePrefix}) {
    final effectivePrefix = tablePrefix != null ? '$tablePrefix.' : '';
    return PendingLocationPing(
      id: attachedDatabase.typeMapping
          .read(DriftSqlType.int, data['${effectivePrefix}id'])!,
      appUserId: attachedDatabase.typeMapping
          .read(DriftSqlType.string, data['${effectivePrefix}app_user_id'])!,
      lat: attachedDatabase.typeMapping
          .read(DriftSqlType.double, data['${effectivePrefix}lat'])!,
      lng: attachedDatabase.typeMapping
          .read(DriftSqlType.double, data['${effectivePrefix}lng'])!,
      accuracy: attachedDatabase.typeMapping
          .read(DriftSqlType.double, data['${effectivePrefix}accuracy']),
      occurredAtClient: attachedDatabase.typeMapping.read(
          DriftSqlType.string, data['${effectivePrefix}occurred_at_client'])!,
      status: attachedDatabase.typeMapping
          .read(DriftSqlType.string, data['${effectivePrefix}status'])!,
      attempts: attachedDatabase.typeMapping
          .read(DriftSqlType.int, data['${effectivePrefix}attempts'])!,
      lastErrorCode: attachedDatabase.typeMapping
          .read(DriftSqlType.string, data['${effectivePrefix}last_error_code']),
      lastErrorMessage: attachedDatabase.typeMapping.read(
          DriftSqlType.string, data['${effectivePrefix}last_error_message']),
      lastAttemptAt: attachedDatabase.typeMapping
          .read(DriftSqlType.string, data['${effectivePrefix}last_attempt_at']),
      enqueuedAt: attachedDatabase.typeMapping
          .read(DriftSqlType.string, data['${effectivePrefix}enqueued_at'])!,
    );
  }

  @override
  $PendingLocationPingsTable createAlias(String alias) {
    return $PendingLocationPingsTable(attachedDatabase, alias);
  }
}

class PendingLocationPing extends DataClass
    implements Insertable<PendingLocationPing> {
  final int id;
  final String appUserId;
  final double lat;
  final double lng;
  final double? accuracy;
  final String occurredAtClient;
  final String status;
  final int attempts;
  final String? lastErrorCode;
  final String? lastErrorMessage;
  final String? lastAttemptAt;
  final String enqueuedAt;
  const PendingLocationPing(
      {required this.id,
      required this.appUserId,
      required this.lat,
      required this.lng,
      this.accuracy,
      required this.occurredAtClient,
      required this.status,
      required this.attempts,
      this.lastErrorCode,
      this.lastErrorMessage,
      this.lastAttemptAt,
      required this.enqueuedAt});
  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    map['id'] = Variable<int>(id);
    map['app_user_id'] = Variable<String>(appUserId);
    map['lat'] = Variable<double>(lat);
    map['lng'] = Variable<double>(lng);
    if (!nullToAbsent || accuracy != null) {
      map['accuracy'] = Variable<double>(accuracy);
    }
    map['occurred_at_client'] = Variable<String>(occurredAtClient);
    map['status'] = Variable<String>(status);
    map['attempts'] = Variable<int>(attempts);
    if (!nullToAbsent || lastErrorCode != null) {
      map['last_error_code'] = Variable<String>(lastErrorCode);
    }
    if (!nullToAbsent || lastErrorMessage != null) {
      map['last_error_message'] = Variable<String>(lastErrorMessage);
    }
    if (!nullToAbsent || lastAttemptAt != null) {
      map['last_attempt_at'] = Variable<String>(lastAttemptAt);
    }
    map['enqueued_at'] = Variable<String>(enqueuedAt);
    return map;
  }

  PendingLocationPingsCompanion toCompanion(bool nullToAbsent) {
    return PendingLocationPingsCompanion(
      id: Value(id),
      appUserId: Value(appUserId),
      lat: Value(lat),
      lng: Value(lng),
      accuracy: accuracy == null && nullToAbsent
          ? const Value.absent()
          : Value(accuracy),
      occurredAtClient: Value(occurredAtClient),
      status: Value(status),
      attempts: Value(attempts),
      lastErrorCode: lastErrorCode == null && nullToAbsent
          ? const Value.absent()
          : Value(lastErrorCode),
      lastErrorMessage: lastErrorMessage == null && nullToAbsent
          ? const Value.absent()
          : Value(lastErrorMessage),
      lastAttemptAt: lastAttemptAt == null && nullToAbsent
          ? const Value.absent()
          : Value(lastAttemptAt),
      enqueuedAt: Value(enqueuedAt),
    );
  }

  factory PendingLocationPing.fromJson(Map<String, dynamic> json,
      {ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return PendingLocationPing(
      id: serializer.fromJson<int>(json['id']),
      appUserId: serializer.fromJson<String>(json['appUserId']),
      lat: serializer.fromJson<double>(json['lat']),
      lng: serializer.fromJson<double>(json['lng']),
      accuracy: serializer.fromJson<double?>(json['accuracy']),
      occurredAtClient: serializer.fromJson<String>(json['occurredAtClient']),
      status: serializer.fromJson<String>(json['status']),
      attempts: serializer.fromJson<int>(json['attempts']),
      lastErrorCode: serializer.fromJson<String?>(json['lastErrorCode']),
      lastErrorMessage: serializer.fromJson<String?>(json['lastErrorMessage']),
      lastAttemptAt: serializer.fromJson<String?>(json['lastAttemptAt']),
      enqueuedAt: serializer.fromJson<String>(json['enqueuedAt']),
    );
  }
  @override
  Map<String, dynamic> toJson({ValueSerializer? serializer}) {
    serializer ??= driftRuntimeOptions.defaultSerializer;
    return <String, dynamic>{
      'id': serializer.toJson<int>(id),
      'appUserId': serializer.toJson<String>(appUserId),
      'lat': serializer.toJson<double>(lat),
      'lng': serializer.toJson<double>(lng),
      'accuracy': serializer.toJson<double?>(accuracy),
      'occurredAtClient': serializer.toJson<String>(occurredAtClient),
      'status': serializer.toJson<String>(status),
      'attempts': serializer.toJson<int>(attempts),
      'lastErrorCode': serializer.toJson<String?>(lastErrorCode),
      'lastErrorMessage': serializer.toJson<String?>(lastErrorMessage),
      'lastAttemptAt': serializer.toJson<String?>(lastAttemptAt),
      'enqueuedAt': serializer.toJson<String>(enqueuedAt),
    };
  }

  PendingLocationPing copyWith(
          {int? id,
          String? appUserId,
          double? lat,
          double? lng,
          Value<double?> accuracy = const Value.absent(),
          String? occurredAtClient,
          String? status,
          int? attempts,
          Value<String?> lastErrorCode = const Value.absent(),
          Value<String?> lastErrorMessage = const Value.absent(),
          Value<String?> lastAttemptAt = const Value.absent(),
          String? enqueuedAt}) =>
      PendingLocationPing(
        id: id ?? this.id,
        appUserId: appUserId ?? this.appUserId,
        lat: lat ?? this.lat,
        lng: lng ?? this.lng,
        accuracy: accuracy.present ? accuracy.value : this.accuracy,
        occurredAtClient: occurredAtClient ?? this.occurredAtClient,
        status: status ?? this.status,
        attempts: attempts ?? this.attempts,
        lastErrorCode:
            lastErrorCode.present ? lastErrorCode.value : this.lastErrorCode,
        lastErrorMessage: lastErrorMessage.present
            ? lastErrorMessage.value
            : this.lastErrorMessage,
        lastAttemptAt:
            lastAttemptAt.present ? lastAttemptAt.value : this.lastAttemptAt,
        enqueuedAt: enqueuedAt ?? this.enqueuedAt,
      );
  PendingLocationPing copyWithCompanion(PendingLocationPingsCompanion data) {
    return PendingLocationPing(
      id: data.id.present ? data.id.value : this.id,
      appUserId: data.appUserId.present ? data.appUserId.value : this.appUserId,
      lat: data.lat.present ? data.lat.value : this.lat,
      lng: data.lng.present ? data.lng.value : this.lng,
      accuracy: data.accuracy.present ? data.accuracy.value : this.accuracy,
      occurredAtClient: data.occurredAtClient.present
          ? data.occurredAtClient.value
          : this.occurredAtClient,
      status: data.status.present ? data.status.value : this.status,
      attempts: data.attempts.present ? data.attempts.value : this.attempts,
      lastErrorCode: data.lastErrorCode.present
          ? data.lastErrorCode.value
          : this.lastErrorCode,
      lastErrorMessage: data.lastErrorMessage.present
          ? data.lastErrorMessage.value
          : this.lastErrorMessage,
      lastAttemptAt: data.lastAttemptAt.present
          ? data.lastAttemptAt.value
          : this.lastAttemptAt,
      enqueuedAt:
          data.enqueuedAt.present ? data.enqueuedAt.value : this.enqueuedAt,
    );
  }

  @override
  String toString() {
    return (StringBuffer('PendingLocationPing(')
          ..write('id: $id, ')
          ..write('appUserId: $appUserId, ')
          ..write('lat: $lat, ')
          ..write('lng: $lng, ')
          ..write('accuracy: $accuracy, ')
          ..write('occurredAtClient: $occurredAtClient, ')
          ..write('status: $status, ')
          ..write('attempts: $attempts, ')
          ..write('lastErrorCode: $lastErrorCode, ')
          ..write('lastErrorMessage: $lastErrorMessage, ')
          ..write('lastAttemptAt: $lastAttemptAt, ')
          ..write('enqueuedAt: $enqueuedAt')
          ..write(')'))
        .toString();
  }

  @override
  int get hashCode => Object.hash(
      id,
      appUserId,
      lat,
      lng,
      accuracy,
      occurredAtClient,
      status,
      attempts,
      lastErrorCode,
      lastErrorMessage,
      lastAttemptAt,
      enqueuedAt);
  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      (other is PendingLocationPing &&
          other.id == this.id &&
          other.appUserId == this.appUserId &&
          other.lat == this.lat &&
          other.lng == this.lng &&
          other.accuracy == this.accuracy &&
          other.occurredAtClient == this.occurredAtClient &&
          other.status == this.status &&
          other.attempts == this.attempts &&
          other.lastErrorCode == this.lastErrorCode &&
          other.lastErrorMessage == this.lastErrorMessage &&
          other.lastAttemptAt == this.lastAttemptAt &&
          other.enqueuedAt == this.enqueuedAt);
}

class PendingLocationPingsCompanion
    extends UpdateCompanion<PendingLocationPing> {
  final Value<int> id;
  final Value<String> appUserId;
  final Value<double> lat;
  final Value<double> lng;
  final Value<double?> accuracy;
  final Value<String> occurredAtClient;
  final Value<String> status;
  final Value<int> attempts;
  final Value<String?> lastErrorCode;
  final Value<String?> lastErrorMessage;
  final Value<String?> lastAttemptAt;
  final Value<String> enqueuedAt;
  const PendingLocationPingsCompanion({
    this.id = const Value.absent(),
    this.appUserId = const Value.absent(),
    this.lat = const Value.absent(),
    this.lng = const Value.absent(),
    this.accuracy = const Value.absent(),
    this.occurredAtClient = const Value.absent(),
    this.status = const Value.absent(),
    this.attempts = const Value.absent(),
    this.lastErrorCode = const Value.absent(),
    this.lastErrorMessage = const Value.absent(),
    this.lastAttemptAt = const Value.absent(),
    this.enqueuedAt = const Value.absent(),
  });
  PendingLocationPingsCompanion.insert({
    this.id = const Value.absent(),
    required String appUserId,
    required double lat,
    required double lng,
    this.accuracy = const Value.absent(),
    required String occurredAtClient,
    this.status = const Value.absent(),
    this.attempts = const Value.absent(),
    this.lastErrorCode = const Value.absent(),
    this.lastErrorMessage = const Value.absent(),
    this.lastAttemptAt = const Value.absent(),
    required String enqueuedAt,
  })  : appUserId = Value(appUserId),
        lat = Value(lat),
        lng = Value(lng),
        occurredAtClient = Value(occurredAtClient),
        enqueuedAt = Value(enqueuedAt);
  static Insertable<PendingLocationPing> custom({
    Expression<int>? id,
    Expression<String>? appUserId,
    Expression<double>? lat,
    Expression<double>? lng,
    Expression<double>? accuracy,
    Expression<String>? occurredAtClient,
    Expression<String>? status,
    Expression<int>? attempts,
    Expression<String>? lastErrorCode,
    Expression<String>? lastErrorMessage,
    Expression<String>? lastAttemptAt,
    Expression<String>? enqueuedAt,
  }) {
    return RawValuesInsertable({
      if (id != null) 'id': id,
      if (appUserId != null) 'app_user_id': appUserId,
      if (lat != null) 'lat': lat,
      if (lng != null) 'lng': lng,
      if (accuracy != null) 'accuracy': accuracy,
      if (occurredAtClient != null) 'occurred_at_client': occurredAtClient,
      if (status != null) 'status': status,
      if (attempts != null) 'attempts': attempts,
      if (lastErrorCode != null) 'last_error_code': lastErrorCode,
      if (lastErrorMessage != null) 'last_error_message': lastErrorMessage,
      if (lastAttemptAt != null) 'last_attempt_at': lastAttemptAt,
      if (enqueuedAt != null) 'enqueued_at': enqueuedAt,
    });
  }

  PendingLocationPingsCompanion copyWith(
      {Value<int>? id,
      Value<String>? appUserId,
      Value<double>? lat,
      Value<double>? lng,
      Value<double?>? accuracy,
      Value<String>? occurredAtClient,
      Value<String>? status,
      Value<int>? attempts,
      Value<String?>? lastErrorCode,
      Value<String?>? lastErrorMessage,
      Value<String?>? lastAttemptAt,
      Value<String>? enqueuedAt}) {
    return PendingLocationPingsCompanion(
      id: id ?? this.id,
      appUserId: appUserId ?? this.appUserId,
      lat: lat ?? this.lat,
      lng: lng ?? this.lng,
      accuracy: accuracy ?? this.accuracy,
      occurredAtClient: occurredAtClient ?? this.occurredAtClient,
      status: status ?? this.status,
      attempts: attempts ?? this.attempts,
      lastErrorCode: lastErrorCode ?? this.lastErrorCode,
      lastErrorMessage: lastErrorMessage ?? this.lastErrorMessage,
      lastAttemptAt: lastAttemptAt ?? this.lastAttemptAt,
      enqueuedAt: enqueuedAt ?? this.enqueuedAt,
    );
  }

  @override
  Map<String, Expression> toColumns(bool nullToAbsent) {
    final map = <String, Expression>{};
    if (id.present) {
      map['id'] = Variable<int>(id.value);
    }
    if (appUserId.present) {
      map['app_user_id'] = Variable<String>(appUserId.value);
    }
    if (lat.present) {
      map['lat'] = Variable<double>(lat.value);
    }
    if (lng.present) {
      map['lng'] = Variable<double>(lng.value);
    }
    if (accuracy.present) {
      map['accuracy'] = Variable<double>(accuracy.value);
    }
    if (occurredAtClient.present) {
      map['occurred_at_client'] = Variable<String>(occurredAtClient.value);
    }
    if (status.present) {
      map['status'] = Variable<String>(status.value);
    }
    if (attempts.present) {
      map['attempts'] = Variable<int>(attempts.value);
    }
    if (lastErrorCode.present) {
      map['last_error_code'] = Variable<String>(lastErrorCode.value);
    }
    if (lastErrorMessage.present) {
      map['last_error_message'] = Variable<String>(lastErrorMessage.value);
    }
    if (lastAttemptAt.present) {
      map['last_attempt_at'] = Variable<String>(lastAttemptAt.value);
    }
    if (enqueuedAt.present) {
      map['enqueued_at'] = Variable<String>(enqueuedAt.value);
    }
    return map;
  }

  @override
  String toString() {
    return (StringBuffer('PendingLocationPingsCompanion(')
          ..write('id: $id, ')
          ..write('appUserId: $appUserId, ')
          ..write('lat: $lat, ')
          ..write('lng: $lng, ')
          ..write('accuracy: $accuracy, ')
          ..write('occurredAtClient: $occurredAtClient, ')
          ..write('status: $status, ')
          ..write('attempts: $attempts, ')
          ..write('lastErrorCode: $lastErrorCode, ')
          ..write('lastErrorMessage: $lastErrorMessage, ')
          ..write('lastAttemptAt: $lastAttemptAt, ')
          ..write('enqueuedAt: $enqueuedAt')
          ..write(')'))
        .toString();
  }
}

abstract class _$CheckinQueueDb extends GeneratedDatabase {
  _$CheckinQueueDb(QueryExecutor e) : super(e);
  $CheckinQueueDbManager get managers => $CheckinQueueDbManager(this);
  late final $PendingEventsTable pendingEvents = $PendingEventsTable(this);
  late final $PendingLocationPingsTable pendingLocationPings =
      $PendingLocationPingsTable(this);
  late final Index idxPendingStatusTime = Index('idx_pending_status_time',
      'CREATE INDEX idx_pending_status_time ON pending_events (status, occurred_at_client)');
  late final Index idxPendingLocStatusTime = Index(
      'idx_pending_loc_status_time',
      'CREATE INDEX idx_pending_loc_status_time ON pending_location_pings (status, occurred_at_client)');
  @override
  Iterable<TableInfo<Table, Object?>> get allTables =>
      allSchemaEntities.whereType<TableInfo<Table, Object?>>();
  @override
  List<DatabaseSchemaEntity> get allSchemaEntities => [
        pendingEvents,
        pendingLocationPings,
        idxPendingStatusTime,
        idxPendingLocStatusTime
      ];
}

typedef $$PendingEventsTableCreateCompanionBuilder = PendingEventsCompanion
    Function({
  Value<int> id,
  required String appUserId,
  required String eventType,
  required double lat,
  required double lng,
  Value<double?> accuracy,
  Value<String?> manualLabel,
  required String occurredAtClient,
  Value<String> status,
  Value<int> attempts,
  Value<String?> lastErrorCode,
  Value<String?> lastErrorMessage,
  Value<String?> lastAttemptAt,
  required String enqueuedAt,
});
typedef $$PendingEventsTableUpdateCompanionBuilder = PendingEventsCompanion
    Function({
  Value<int> id,
  Value<String> appUserId,
  Value<String> eventType,
  Value<double> lat,
  Value<double> lng,
  Value<double?> accuracy,
  Value<String?> manualLabel,
  Value<String> occurredAtClient,
  Value<String> status,
  Value<int> attempts,
  Value<String?> lastErrorCode,
  Value<String?> lastErrorMessage,
  Value<String?> lastAttemptAt,
  Value<String> enqueuedAt,
});

class $$PendingEventsTableFilterComposer
    extends Composer<_$CheckinQueueDb, $PendingEventsTable> {
  $$PendingEventsTableFilterComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnFilters<int> get id => $composableBuilder(
      column: $table.id, builder: (column) => ColumnFilters(column));

  ColumnFilters<String> get appUserId => $composableBuilder(
      column: $table.appUserId, builder: (column) => ColumnFilters(column));

  ColumnFilters<String> get eventType => $composableBuilder(
      column: $table.eventType, builder: (column) => ColumnFilters(column));

  ColumnFilters<double> get lat => $composableBuilder(
      column: $table.lat, builder: (column) => ColumnFilters(column));

  ColumnFilters<double> get lng => $composableBuilder(
      column: $table.lng, builder: (column) => ColumnFilters(column));

  ColumnFilters<double> get accuracy => $composableBuilder(
      column: $table.accuracy, builder: (column) => ColumnFilters(column));

  ColumnFilters<String> get manualLabel => $composableBuilder(
      column: $table.manualLabel, builder: (column) => ColumnFilters(column));

  ColumnFilters<String> get occurredAtClient => $composableBuilder(
      column: $table.occurredAtClient,
      builder: (column) => ColumnFilters(column));

  ColumnFilters<String> get status => $composableBuilder(
      column: $table.status, builder: (column) => ColumnFilters(column));

  ColumnFilters<int> get attempts => $composableBuilder(
      column: $table.attempts, builder: (column) => ColumnFilters(column));

  ColumnFilters<String> get lastErrorCode => $composableBuilder(
      column: $table.lastErrorCode, builder: (column) => ColumnFilters(column));

  ColumnFilters<String> get lastErrorMessage => $composableBuilder(
      column: $table.lastErrorMessage,
      builder: (column) => ColumnFilters(column));

  ColumnFilters<String> get lastAttemptAt => $composableBuilder(
      column: $table.lastAttemptAt, builder: (column) => ColumnFilters(column));

  ColumnFilters<String> get enqueuedAt => $composableBuilder(
      column: $table.enqueuedAt, builder: (column) => ColumnFilters(column));
}

class $$PendingEventsTableOrderingComposer
    extends Composer<_$CheckinQueueDb, $PendingEventsTable> {
  $$PendingEventsTableOrderingComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnOrderings<int> get id => $composableBuilder(
      column: $table.id, builder: (column) => ColumnOrderings(column));

  ColumnOrderings<String> get appUserId => $composableBuilder(
      column: $table.appUserId, builder: (column) => ColumnOrderings(column));

  ColumnOrderings<String> get eventType => $composableBuilder(
      column: $table.eventType, builder: (column) => ColumnOrderings(column));

  ColumnOrderings<double> get lat => $composableBuilder(
      column: $table.lat, builder: (column) => ColumnOrderings(column));

  ColumnOrderings<double> get lng => $composableBuilder(
      column: $table.lng, builder: (column) => ColumnOrderings(column));

  ColumnOrderings<double> get accuracy => $composableBuilder(
      column: $table.accuracy, builder: (column) => ColumnOrderings(column));

  ColumnOrderings<String> get manualLabel => $composableBuilder(
      column: $table.manualLabel, builder: (column) => ColumnOrderings(column));

  ColumnOrderings<String> get occurredAtClient => $composableBuilder(
      column: $table.occurredAtClient,
      builder: (column) => ColumnOrderings(column));

  ColumnOrderings<String> get status => $composableBuilder(
      column: $table.status, builder: (column) => ColumnOrderings(column));

  ColumnOrderings<int> get attempts => $composableBuilder(
      column: $table.attempts, builder: (column) => ColumnOrderings(column));

  ColumnOrderings<String> get lastErrorCode => $composableBuilder(
      column: $table.lastErrorCode,
      builder: (column) => ColumnOrderings(column));

  ColumnOrderings<String> get lastErrorMessage => $composableBuilder(
      column: $table.lastErrorMessage,
      builder: (column) => ColumnOrderings(column));

  ColumnOrderings<String> get lastAttemptAt => $composableBuilder(
      column: $table.lastAttemptAt,
      builder: (column) => ColumnOrderings(column));

  ColumnOrderings<String> get enqueuedAt => $composableBuilder(
      column: $table.enqueuedAt, builder: (column) => ColumnOrderings(column));
}

class $$PendingEventsTableAnnotationComposer
    extends Composer<_$CheckinQueueDb, $PendingEventsTable> {
  $$PendingEventsTableAnnotationComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  GeneratedColumn<int> get id =>
      $composableBuilder(column: $table.id, builder: (column) => column);

  GeneratedColumn<String> get appUserId =>
      $composableBuilder(column: $table.appUserId, builder: (column) => column);

  GeneratedColumn<String> get eventType =>
      $composableBuilder(column: $table.eventType, builder: (column) => column);

  GeneratedColumn<double> get lat =>
      $composableBuilder(column: $table.lat, builder: (column) => column);

  GeneratedColumn<double> get lng =>
      $composableBuilder(column: $table.lng, builder: (column) => column);

  GeneratedColumn<double> get accuracy =>
      $composableBuilder(column: $table.accuracy, builder: (column) => column);

  GeneratedColumn<String> get manualLabel => $composableBuilder(
      column: $table.manualLabel, builder: (column) => column);

  GeneratedColumn<String> get occurredAtClient => $composableBuilder(
      column: $table.occurredAtClient, builder: (column) => column);

  GeneratedColumn<String> get status =>
      $composableBuilder(column: $table.status, builder: (column) => column);

  GeneratedColumn<int> get attempts =>
      $composableBuilder(column: $table.attempts, builder: (column) => column);

  GeneratedColumn<String> get lastErrorCode => $composableBuilder(
      column: $table.lastErrorCode, builder: (column) => column);

  GeneratedColumn<String> get lastErrorMessage => $composableBuilder(
      column: $table.lastErrorMessage, builder: (column) => column);

  GeneratedColumn<String> get lastAttemptAt => $composableBuilder(
      column: $table.lastAttemptAt, builder: (column) => column);

  GeneratedColumn<String> get enqueuedAt => $composableBuilder(
      column: $table.enqueuedAt, builder: (column) => column);
}

class $$PendingEventsTableTableManager extends RootTableManager<
    _$CheckinQueueDb,
    $PendingEventsTable,
    PendingEvent,
    $$PendingEventsTableFilterComposer,
    $$PendingEventsTableOrderingComposer,
    $$PendingEventsTableAnnotationComposer,
    $$PendingEventsTableCreateCompanionBuilder,
    $$PendingEventsTableUpdateCompanionBuilder,
    (
      PendingEvent,
      BaseReferences<_$CheckinQueueDb, $PendingEventsTable, PendingEvent>
    ),
    PendingEvent,
    PrefetchHooks Function()> {
  $$PendingEventsTableTableManager(
      _$CheckinQueueDb db, $PendingEventsTable table)
      : super(TableManagerState(
          db: db,
          table: table,
          createFilteringComposer: () =>
              $$PendingEventsTableFilterComposer($db: db, $table: table),
          createOrderingComposer: () =>
              $$PendingEventsTableOrderingComposer($db: db, $table: table),
          createComputedFieldComposer: () =>
              $$PendingEventsTableAnnotationComposer($db: db, $table: table),
          updateCompanionCallback: ({
            Value<int> id = const Value.absent(),
            Value<String> appUserId = const Value.absent(),
            Value<String> eventType = const Value.absent(),
            Value<double> lat = const Value.absent(),
            Value<double> lng = const Value.absent(),
            Value<double?> accuracy = const Value.absent(),
            Value<String?> manualLabel = const Value.absent(),
            Value<String> occurredAtClient = const Value.absent(),
            Value<String> status = const Value.absent(),
            Value<int> attempts = const Value.absent(),
            Value<String?> lastErrorCode = const Value.absent(),
            Value<String?> lastErrorMessage = const Value.absent(),
            Value<String?> lastAttemptAt = const Value.absent(),
            Value<String> enqueuedAt = const Value.absent(),
          }) =>
              PendingEventsCompanion(
            id: id,
            appUserId: appUserId,
            eventType: eventType,
            lat: lat,
            lng: lng,
            accuracy: accuracy,
            manualLabel: manualLabel,
            occurredAtClient: occurredAtClient,
            status: status,
            attempts: attempts,
            lastErrorCode: lastErrorCode,
            lastErrorMessage: lastErrorMessage,
            lastAttemptAt: lastAttemptAt,
            enqueuedAt: enqueuedAt,
          ),
          createCompanionCallback: ({
            Value<int> id = const Value.absent(),
            required String appUserId,
            required String eventType,
            required double lat,
            required double lng,
            Value<double?> accuracy = const Value.absent(),
            Value<String?> manualLabel = const Value.absent(),
            required String occurredAtClient,
            Value<String> status = const Value.absent(),
            Value<int> attempts = const Value.absent(),
            Value<String?> lastErrorCode = const Value.absent(),
            Value<String?> lastErrorMessage = const Value.absent(),
            Value<String?> lastAttemptAt = const Value.absent(),
            required String enqueuedAt,
          }) =>
              PendingEventsCompanion.insert(
            id: id,
            appUserId: appUserId,
            eventType: eventType,
            lat: lat,
            lng: lng,
            accuracy: accuracy,
            manualLabel: manualLabel,
            occurredAtClient: occurredAtClient,
            status: status,
            attempts: attempts,
            lastErrorCode: lastErrorCode,
            lastErrorMessage: lastErrorMessage,
            lastAttemptAt: lastAttemptAt,
            enqueuedAt: enqueuedAt,
          ),
          withReferenceMapper: (p0) => p0
              .map((e) => (e.readTable(table), BaseReferences(db, table, e)))
              .toList(),
          prefetchHooksCallback: null,
        ));
}

typedef $$PendingEventsTableProcessedTableManager = ProcessedTableManager<
    _$CheckinQueueDb,
    $PendingEventsTable,
    PendingEvent,
    $$PendingEventsTableFilterComposer,
    $$PendingEventsTableOrderingComposer,
    $$PendingEventsTableAnnotationComposer,
    $$PendingEventsTableCreateCompanionBuilder,
    $$PendingEventsTableUpdateCompanionBuilder,
    (
      PendingEvent,
      BaseReferences<_$CheckinQueueDb, $PendingEventsTable, PendingEvent>
    ),
    PendingEvent,
    PrefetchHooks Function()>;
typedef $$PendingLocationPingsTableCreateCompanionBuilder
    = PendingLocationPingsCompanion Function({
  Value<int> id,
  required String appUserId,
  required double lat,
  required double lng,
  Value<double?> accuracy,
  required String occurredAtClient,
  Value<String> status,
  Value<int> attempts,
  Value<String?> lastErrorCode,
  Value<String?> lastErrorMessage,
  Value<String?> lastAttemptAt,
  required String enqueuedAt,
});
typedef $$PendingLocationPingsTableUpdateCompanionBuilder
    = PendingLocationPingsCompanion Function({
  Value<int> id,
  Value<String> appUserId,
  Value<double> lat,
  Value<double> lng,
  Value<double?> accuracy,
  Value<String> occurredAtClient,
  Value<String> status,
  Value<int> attempts,
  Value<String?> lastErrorCode,
  Value<String?> lastErrorMessage,
  Value<String?> lastAttemptAt,
  Value<String> enqueuedAt,
});

class $$PendingLocationPingsTableFilterComposer
    extends Composer<_$CheckinQueueDb, $PendingLocationPingsTable> {
  $$PendingLocationPingsTableFilterComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnFilters<int> get id => $composableBuilder(
      column: $table.id, builder: (column) => ColumnFilters(column));

  ColumnFilters<String> get appUserId => $composableBuilder(
      column: $table.appUserId, builder: (column) => ColumnFilters(column));

  ColumnFilters<double> get lat => $composableBuilder(
      column: $table.lat, builder: (column) => ColumnFilters(column));

  ColumnFilters<double> get lng => $composableBuilder(
      column: $table.lng, builder: (column) => ColumnFilters(column));

  ColumnFilters<double> get accuracy => $composableBuilder(
      column: $table.accuracy, builder: (column) => ColumnFilters(column));

  ColumnFilters<String> get occurredAtClient => $composableBuilder(
      column: $table.occurredAtClient,
      builder: (column) => ColumnFilters(column));

  ColumnFilters<String> get status => $composableBuilder(
      column: $table.status, builder: (column) => ColumnFilters(column));

  ColumnFilters<int> get attempts => $composableBuilder(
      column: $table.attempts, builder: (column) => ColumnFilters(column));

  ColumnFilters<String> get lastErrorCode => $composableBuilder(
      column: $table.lastErrorCode, builder: (column) => ColumnFilters(column));

  ColumnFilters<String> get lastErrorMessage => $composableBuilder(
      column: $table.lastErrorMessage,
      builder: (column) => ColumnFilters(column));

  ColumnFilters<String> get lastAttemptAt => $composableBuilder(
      column: $table.lastAttemptAt, builder: (column) => ColumnFilters(column));

  ColumnFilters<String> get enqueuedAt => $composableBuilder(
      column: $table.enqueuedAt, builder: (column) => ColumnFilters(column));
}

class $$PendingLocationPingsTableOrderingComposer
    extends Composer<_$CheckinQueueDb, $PendingLocationPingsTable> {
  $$PendingLocationPingsTableOrderingComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  ColumnOrderings<int> get id => $composableBuilder(
      column: $table.id, builder: (column) => ColumnOrderings(column));

  ColumnOrderings<String> get appUserId => $composableBuilder(
      column: $table.appUserId, builder: (column) => ColumnOrderings(column));

  ColumnOrderings<double> get lat => $composableBuilder(
      column: $table.lat, builder: (column) => ColumnOrderings(column));

  ColumnOrderings<double> get lng => $composableBuilder(
      column: $table.lng, builder: (column) => ColumnOrderings(column));

  ColumnOrderings<double> get accuracy => $composableBuilder(
      column: $table.accuracy, builder: (column) => ColumnOrderings(column));

  ColumnOrderings<String> get occurredAtClient => $composableBuilder(
      column: $table.occurredAtClient,
      builder: (column) => ColumnOrderings(column));

  ColumnOrderings<String> get status => $composableBuilder(
      column: $table.status, builder: (column) => ColumnOrderings(column));

  ColumnOrderings<int> get attempts => $composableBuilder(
      column: $table.attempts, builder: (column) => ColumnOrderings(column));

  ColumnOrderings<String> get lastErrorCode => $composableBuilder(
      column: $table.lastErrorCode,
      builder: (column) => ColumnOrderings(column));

  ColumnOrderings<String> get lastErrorMessage => $composableBuilder(
      column: $table.lastErrorMessage,
      builder: (column) => ColumnOrderings(column));

  ColumnOrderings<String> get lastAttemptAt => $composableBuilder(
      column: $table.lastAttemptAt,
      builder: (column) => ColumnOrderings(column));

  ColumnOrderings<String> get enqueuedAt => $composableBuilder(
      column: $table.enqueuedAt, builder: (column) => ColumnOrderings(column));
}

class $$PendingLocationPingsTableAnnotationComposer
    extends Composer<_$CheckinQueueDb, $PendingLocationPingsTable> {
  $$PendingLocationPingsTableAnnotationComposer({
    required super.$db,
    required super.$table,
    super.joinBuilder,
    super.$addJoinBuilderToRootComposer,
    super.$removeJoinBuilderFromRootComposer,
  });
  GeneratedColumn<int> get id =>
      $composableBuilder(column: $table.id, builder: (column) => column);

  GeneratedColumn<String> get appUserId =>
      $composableBuilder(column: $table.appUserId, builder: (column) => column);

  GeneratedColumn<double> get lat =>
      $composableBuilder(column: $table.lat, builder: (column) => column);

  GeneratedColumn<double> get lng =>
      $composableBuilder(column: $table.lng, builder: (column) => column);

  GeneratedColumn<double> get accuracy =>
      $composableBuilder(column: $table.accuracy, builder: (column) => column);

  GeneratedColumn<String> get occurredAtClient => $composableBuilder(
      column: $table.occurredAtClient, builder: (column) => column);

  GeneratedColumn<String> get status =>
      $composableBuilder(column: $table.status, builder: (column) => column);

  GeneratedColumn<int> get attempts =>
      $composableBuilder(column: $table.attempts, builder: (column) => column);

  GeneratedColumn<String> get lastErrorCode => $composableBuilder(
      column: $table.lastErrorCode, builder: (column) => column);

  GeneratedColumn<String> get lastErrorMessage => $composableBuilder(
      column: $table.lastErrorMessage, builder: (column) => column);

  GeneratedColumn<String> get lastAttemptAt => $composableBuilder(
      column: $table.lastAttemptAt, builder: (column) => column);

  GeneratedColumn<String> get enqueuedAt => $composableBuilder(
      column: $table.enqueuedAt, builder: (column) => column);
}

class $$PendingLocationPingsTableTableManager extends RootTableManager<
    _$CheckinQueueDb,
    $PendingLocationPingsTable,
    PendingLocationPing,
    $$PendingLocationPingsTableFilterComposer,
    $$PendingLocationPingsTableOrderingComposer,
    $$PendingLocationPingsTableAnnotationComposer,
    $$PendingLocationPingsTableCreateCompanionBuilder,
    $$PendingLocationPingsTableUpdateCompanionBuilder,
    (
      PendingLocationPing,
      BaseReferences<_$CheckinQueueDb, $PendingLocationPingsTable,
          PendingLocationPing>
    ),
    PendingLocationPing,
    PrefetchHooks Function()> {
  $$PendingLocationPingsTableTableManager(
      _$CheckinQueueDb db, $PendingLocationPingsTable table)
      : super(TableManagerState(
          db: db,
          table: table,
          createFilteringComposer: () =>
              $$PendingLocationPingsTableFilterComposer($db: db, $table: table),
          createOrderingComposer: () =>
              $$PendingLocationPingsTableOrderingComposer(
                  $db: db, $table: table),
          createComputedFieldComposer: () =>
              $$PendingLocationPingsTableAnnotationComposer(
                  $db: db, $table: table),
          updateCompanionCallback: ({
            Value<int> id = const Value.absent(),
            Value<String> appUserId = const Value.absent(),
            Value<double> lat = const Value.absent(),
            Value<double> lng = const Value.absent(),
            Value<double?> accuracy = const Value.absent(),
            Value<String> occurredAtClient = const Value.absent(),
            Value<String> status = const Value.absent(),
            Value<int> attempts = const Value.absent(),
            Value<String?> lastErrorCode = const Value.absent(),
            Value<String?> lastErrorMessage = const Value.absent(),
            Value<String?> lastAttemptAt = const Value.absent(),
            Value<String> enqueuedAt = const Value.absent(),
          }) =>
              PendingLocationPingsCompanion(
            id: id,
            appUserId: appUserId,
            lat: lat,
            lng: lng,
            accuracy: accuracy,
            occurredAtClient: occurredAtClient,
            status: status,
            attempts: attempts,
            lastErrorCode: lastErrorCode,
            lastErrorMessage: lastErrorMessage,
            lastAttemptAt: lastAttemptAt,
            enqueuedAt: enqueuedAt,
          ),
          createCompanionCallback: ({
            Value<int> id = const Value.absent(),
            required String appUserId,
            required double lat,
            required double lng,
            Value<double?> accuracy = const Value.absent(),
            required String occurredAtClient,
            Value<String> status = const Value.absent(),
            Value<int> attempts = const Value.absent(),
            Value<String?> lastErrorCode = const Value.absent(),
            Value<String?> lastErrorMessage = const Value.absent(),
            Value<String?> lastAttemptAt = const Value.absent(),
            required String enqueuedAt,
          }) =>
              PendingLocationPingsCompanion.insert(
            id: id,
            appUserId: appUserId,
            lat: lat,
            lng: lng,
            accuracy: accuracy,
            occurredAtClient: occurredAtClient,
            status: status,
            attempts: attempts,
            lastErrorCode: lastErrorCode,
            lastErrorMessage: lastErrorMessage,
            lastAttemptAt: lastAttemptAt,
            enqueuedAt: enqueuedAt,
          ),
          withReferenceMapper: (p0) => p0
              .map((e) => (e.readTable(table), BaseReferences(db, table, e)))
              .toList(),
          prefetchHooksCallback: null,
        ));
}

typedef $$PendingLocationPingsTableProcessedTableManager
    = ProcessedTableManager<
        _$CheckinQueueDb,
        $PendingLocationPingsTable,
        PendingLocationPing,
        $$PendingLocationPingsTableFilterComposer,
        $$PendingLocationPingsTableOrderingComposer,
        $$PendingLocationPingsTableAnnotationComposer,
        $$PendingLocationPingsTableCreateCompanionBuilder,
        $$PendingLocationPingsTableUpdateCompanionBuilder,
        (
          PendingLocationPing,
          BaseReferences<_$CheckinQueueDb, $PendingLocationPingsTable,
              PendingLocationPing>
        ),
        PendingLocationPing,
        PrefetchHooks Function()>;

class $CheckinQueueDbManager {
  final _$CheckinQueueDb _db;
  $CheckinQueueDbManager(this._db);
  $$PendingEventsTableTableManager get pendingEvents =>
      $$PendingEventsTableTableManager(_db, _db.pendingEvents);
  $$PendingLocationPingsTableTableManager get pendingLocationPings =>
      $$PendingLocationPingsTableTableManager(_db, _db.pendingLocationPings);
}
