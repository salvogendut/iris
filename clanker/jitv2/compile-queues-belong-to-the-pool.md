# Both compilation queues belong to their JIT pool

Upstream `5d13b02c` owns the implementation and the
`a_hot_request_stays_with_its_own_pool` drain/isolation regression. Use that
implementation when merging; there is no separate fork queue implementation.

The ordinary FIFO and priority lane are per-pool. Requests carry raw page
pointers, so an old machine's worker must never compile a new machine's request
against its old bus. Stop and flush drain only their own queues.

The fork keeps one additional worker regression,
`compile_worker_cannot_take_another_pools_hot_request`: FIFO completion proves
that an old worker had the opportunity to consume a replacement's priority
request but left it queued. The replacement then compiles that request using
its own different bus. This tests worker execution in addition to upstream's
queue-draining check.

Original reproduction and validation are recorded in commit `6dc5676a`:
Prismatic's diskless Indy could panic after a PTY-to-TCP restart with JIT
active. Per-pool queues fixed that; they do not resolve device graphs retained
past machine shutdown.
