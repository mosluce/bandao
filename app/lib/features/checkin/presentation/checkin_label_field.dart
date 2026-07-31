import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../l10n/app_localizations.dart';
import '../state/checkin_label_provider.dart';
import '../state/recent_labels_provider.dart';

/// The location label attached to every check-in, with the worker's own most
/// used labels offered as one-tap chips.
///
/// Sits ABOVE the action buttons on purpose. Collecting the label here rather
/// than in a sheet after the press is what keeps the tap→enqueue path free of
/// any dialog — the buttons are simply disabled until this is filled, the
/// same way the login form gates its submit.
class CheckinLabelField extends ConsumerStatefulWidget {
  const CheckinLabelField({super.key});

  @override
  ConsumerState<CheckinLabelField> createState() => _CheckinLabelFieldState();
}

class _CheckinLabelFieldState extends ConsumerState<CheckinLabelField> {
  late final TextEditingController _controller;

  @override
  void initState() {
    super.initState();
    _controller = TextEditingController(text: ref.read(checkinLabelProvider));
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final recent = ref.watch(recentLabelsProvider).valueOrNull ?? const [];
    final value = ref.watch(checkinLabelProvider);

    // `enqueueEvent` clears the provider after a successful submit; mirror
    // that into the controller so the field visibly empties.
    if (value.isEmpty && _controller.text.isNotEmpty) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && ref.read(checkinLabelProvider).isEmpty) {
          _controller.clear();
        }
      });
    }

    final tooLong = value.trim().runes.length > checkinLabelMaxLength;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        TextField(
          key: const Key('checkin.label'),
          controller: _controller,
          textInputAction: TextInputAction.done,
          decoration: InputDecoration(
            labelText: '${l10n.checkinLabelTitle} *',
            hintText: l10n.checkinLabelHint,
            border: const OutlineInputBorder(),
            isDense: true,
            errorText: tooLong ? l10n.checkinLabelTooLong : null,
          ),
          onChanged: (v) =>
              ref.read(checkinLabelProvider.notifier).state = v,
        ),
        if (recent.isNotEmpty) ...<Widget>[
          const SizedBox(height: 8),
          Wrap(
            spacing: 8,
            runSpacing: 4,
            children: <Widget>[
              for (final label in recent)
                ActionChip(
                  key: Key('checkin.label.chip.$label'),
                  label: Text(label),
                  // Fills the field; never submits. The action buttons stay
                  // the only way to record a check-in.
                  onPressed: () {
                    _controller.text = label;
                    _controller.selection = TextSelection.collapsed(
                      offset: label.length,
                    );
                    ref.read(checkinLabelProvider.notifier).state = label;
                  },
                ),
            ],
          ),
        ],
      ],
    );
  }
}
