import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/api/api_error.dart';
import '../../../l10n/app_localizations.dart';
import '../state/auth_provider.dart';

class ForcePasswordChangeScreen extends ConsumerStatefulWidget {
  const ForcePasswordChangeScreen({super.key});

  @override
  ConsumerState<ForcePasswordChangeScreen> createState() =>
      _ForcePasswordChangeScreenState();
}

class _ForcePasswordChangeScreenState
    extends ConsumerState<ForcePasswordChangeScreen> {
  final TextEditingController _current = TextEditingController();
  final TextEditingController _next = TextEditingController();

  bool _submitting = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    _current.addListener(_onChanged);
    _next.addListener(_onChanged);
  }

  @override
  void dispose() {
    _current.dispose();
    _next.dispose();
    super.dispose();
  }

  void _onChanged() => setState(() {});

  bool get _canSubmit {
    if (_submitting) return false;
    return _current.text.isNotEmpty && _next.text.length >= 8;
  }

  Future<void> _submit() async {
    if (!_canSubmit) return;
    setState(() {
      _submitting = true;
      _error = null;
    });
    try {
      await ref.read(authProvider.notifier).changePassword(
            currentPassword: _current.text,
            newPassword: _next.text,
          );
      // Router redirect handles navigation once the flag is cleared.
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() => _error = e.friendlyZh(context));
    } catch (_) {
      if (!mounted) return;
      setState(() => _error = AppLocalizations.of(context).errorGeneric);
    } finally {
      if (mounted) {
        setState(() => _submitting = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    return PopScope(
      // Disable any "back" affordance — the redirect logic also catches
      // programmatic exits, but blocking the system back here keeps the UX
      // explicit.
      canPop: false,
      child: Scaffold(
        // No AppBar back button by design.
        appBar: AppBar(
          automaticallyImplyLeading: false,
          title: Text(l10n.forceChangeTitle),
        ),
        body: SafeArea(
          child: SingleChildScrollView(
            padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 32),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                TextField(
                  key: const Key('forceChange.current'),
                  controller: _current,
                  enabled: !_submitting,
                  obscureText: true,
                  decoration: InputDecoration(
                    labelText: l10n.forceChangeCurrentPasswordLabel,
                    border: const OutlineInputBorder(),
                  ),
                  textInputAction: TextInputAction.next,
                ),
                const SizedBox(height: 16),
                TextField(
                  key: const Key('forceChange.next'),
                  controller: _next,
                  enabled: !_submitting,
                  obscureText: true,
                  decoration: InputDecoration(
                    labelText: l10n.forceChangeNewPasswordLabel,
                    helperText: l10n.forceChangeNewPasswordHint,
                    border: const OutlineInputBorder(),
                  ),
                  textInputAction: TextInputAction.done,
                  onSubmitted: (_) => _submit(),
                ),
                if (_error != null) ...<Widget>[
                  const SizedBox(height: 12),
                  Text(
                    _error!,
                    style:
                        TextStyle(color: Theme.of(context).colorScheme.error),
                  ),
                ],
                const SizedBox(height: 24),
                FilledButton(
                  key: const Key('forceChange.submit'),
                  onPressed: _canSubmit ? _submit : null,
                  child: _submitting
                      ? const SizedBox(
                          height: 20,
                          width: 20,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : Text(l10n.forceChangeSubmit),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
