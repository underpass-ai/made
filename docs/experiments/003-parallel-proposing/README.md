# 003 — Bounded parallel proposing

Initial proposals can overlap when the provider has spare capacity. Once its
serving capacity is full, extra calls only queue. This experiment measures the
application scheduler with a controlled local fixture; it does not measure an
LLM, network service, token throughput or production latency.

`DeliberateUseCase::with_proposal_parallelism(MaxParallel)` opts into a shared
call limit for initial proposals. The semaphore belongs to the use case, so
simultaneous councils through that instance share the limit. Separate use-case
instances and processes have separate pools. Constructors, the service and the
embedded defaults continue to propose sequentially. Peer review stays ordered.
No environment variable enables this experiment automatically.

The opt-in drains provider futures before returning a proposal error and keeps
results in agent declaration order. Cancelling the entire caller drops local
futures and releases permits; this does not promise cancellation of a remote
provider's work. The default sequential path still stops on its first error.

## Method and evidence

Measured code: `86460f6b784a23e7a39d72cb35b30394a03433d6`.
[Environment](results/environment.json) records the exact tree and toolchain;
[measurements](results/measurements.csv) contain all 40 samples.

Eight fixture agents each wait 20 ms while holding a provider serving slot.
The provider has either one or four slots. Scheduler widths are 1 (the actual
sequential helper), 2, 4 and 8, with five runs per combination. This is a debug
build on Tokio's current-thread test runtime; other development builds were
running on the host. Compilation time is excluded. The experiment asserts all
eight proposals finish but makes no timing assertion in ordinary tests.

| Provider slots | Width 1 median ms | Width 2 | Width 4 | Width 8 |
| --- | ---: | ---: | ---: | ---: |
| 1 | 169.850 | 170.313 | 169.218 | 169.282 |
| 4 | 169.746 | 84.879 | 42.563 | 42.575 |

Four serving slots reduce proposal time by about four times at width 4 in this
fixture. Width 8 adds no useful throughput. With one serving slot, all widths
take about 170 ms. Peak outstanding application calls equals the requested
width, including calls queued at the provider; it is not a count of GPUs or
actively generated responses.

The decision is to retain sequential defaults. A host may opt in after measuring
its actual provider, concurrent ceremony workload, rate limits and error cost.
This fixture establishes scheduling behavior and saturation, not a production
width recommendation. Validation/scoring, peer review and persistence are not
included in these timings.

## Repeat without overwriting this evidence

Run `./run.sh /absolute/path/to/new-evidence` from this repository. The script
refuses an existing output directory and records a new revision/environment.
Ordinary tests cover shared limits across concurrent councils, reverse-order
completion, failure draining, caller cancellation and the complete deliberation
path with the opt-in enabled.
