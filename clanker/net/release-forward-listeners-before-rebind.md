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
