Nether C is planned as a series of descents. A descent lands when its *proof*
runs — not when its code exists. Every item below carries the observable fact
that settles it.

Statuses are the language's own: work is `descending` while it is underway,
`starved` when it is waiting on something only somebody else can supply, and
`buried` when it is evaluated as far as the world currently allows.

## Reading the remaining work

Open items carry a dated delivery plan: the current starting point, ordered
steps, scope limits and evidence needed to close. That plan supersedes stale
assumptions in the original proposal; dated observations remain history, not
fresh verification. A plan is not a completed proof.

Milestone descriptions explain sequencing. `depends_on` is a hard gate;
preparatory work described in an item is not permission to bypass it. Due dates
are roadmap targets, not commitments. Large research items must identify and
file missing prerequisites before implementation, rather than hide them in an
unbounded PR.

Start with making the encoding's invalid states unconstructible (0115) and
finishing the release/specimen path for the face (0235); both are bounded and
unblocked. Implementation-neutral conformance prep (0105) is also ready, while
burying it waits on an independent second implementation. Arrange the
outstanding browser observation (0075) separately. The current editor and
interpreter bootstrap are already delivered; their remaining items name the
actual gaps. The year-long store study (0210), independent conformance
implementation (0105) and human studies remain external evidence gates, not
things a passing build can settle.
