# Release old forwarding sockets before rebinding their ports

Prismatic's live Network save exposed `Address already in use` when changing
the bind scope of an existing forward. Editing a rule set could also lose
unchanged forwards whose host ports appeared in both sets.

`NatEngine::rebind_forwards` split off transient FTP listeners but left the
static TCP listeners and UDP sockets alive during `bind_forwards`. They were
only dropped when assigning the replacement vectors, after binding had failed.

Clear both old static listener collections before binding replacements. Keep
the split-off transient listeners and established connection maps untouched.
The directed `live_forward_rebind_reuses_ports_and_preserves_transient_listeners`
test covers repeated TCP/UDP ports, guest-port changes, both bind scopes, removal
and transient FTP listeners. Test-created transient listeners must be nonblocking,
just like the actual FTP path, because an engine pass polls their `accept` loop.

The same release-before-bind ordering applies to renewed `Link`s: SEEQ can
restart before its detached old backend observes the stop flag. Renewed links
share the worker handle and join that run before constructing a replacement.
Do not stop a backend simply because its Link was consumed into Rings: MEC
intentionally starts a single detached backend this way.

Keep explicit live choices independently of the notification flags: a backend
consumes those flags, but a future renewed link still needs the chosen subnet,
forward rules and PCAP interface. `NatControl::runtime_config` reapplies them at
backend construction. Empty forwards and automatic interface are choices too.
