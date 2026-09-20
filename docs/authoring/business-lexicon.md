# Ceremony business vocabulary

Business roles describe responsibility. A role's name does not grant authority;
the definition, guards and scoped authorization determine permitted actions.

## Integrator (integrador)

The **Integrator** is the ceremony role responsible for bringing participants'
contributions into a coherent joint result, resolving dependencies between
roles and checking the evidence required to propose progression or closure.

The integrator:

- Maintains the overall view of scope, agreed contracts and unresolved blockers.
- Coordinates ownership, dependencies and handoffs between contributors.
- Assembles their outputs and checks the combined result against acceptance
  criteria, preserving the attribution of each contribution and review.
- Proposes progression, or applies a transition when authorized, only when its
  required evidence and guards are satisfied.
- Explains the joint result, remaining work and decisions needed from the user.

The responsibility does not confer permission to bypass a guard, replace a
required independent review, approve on behalf of a person, or declare
unverified work complete. A finished contribution is not necessarily an
accepted integrated result.

A person or an agent may fill the role as declared by the ceremony. Its model
and concrete executor are execution choices: a handoff may change those while
preserving the responsibility, constraints and provenance of the work.

For example, a worker contributor and a connector contributor can each finish
their implementation. The integrator checks that their interfaces agree and
that the combined execution passes the required acceptance scenarios. Any
independent reviewer or human approval required by the definition still acts
in its own role.

## Related responsibilities

| Term | Responsibility |
|:--|:--|
| Contributor | Produces an assigned result and supplies evidence for it. |
| Reviewer | Assesses a result against stated criteria; required independence must be preserved. |
| Integrator | Brings contributions together and accounts for the joint result. |
| Host | Provisions actual executors and carries out delegated work through its integrations. |
| Engine | Validates ceremony commands and persists the resulting state and events. |

The host and engine describe execution responsibilities; they do not replace
the business roles declared in a ceremony.

## Authoring and current support

Use an explicit role such as `integrator` when the work needs this
responsibility. Give it concrete steps, outputs and permitted actions through
the existing role mechanisms. Keep review and human decisions explicit.
Simple ceremonies need not introduce an integrator role.

This vocabulary does not add a reserved role id, actor kind, automatic
assignment or special runtime privilege. The executable
[Integrator delivery example](examples/integrator-delivery.yaml) and the
designer, runtime and `made_get_help` guidance show the supported shape;
naming the role alone does not implement live agent visibility, activity
reporting or intervention delivery.

## System vocabulary

An **agentic system** is the named level above one ceremony: business roles,
logical participants, a collaboration topology, several published ceremonies
composed with dependencies and bounded loops, a supervision policy and an
attention policy for the integrator. It references ceremonies through a pin of
name, version and digest instead of holding copies of them. See
[agentic systems](agentic-systems.md).

A **logical participant** is somebody or something the system expects to take
part, described by what it must be able to do rather than by who it is: the role
it plays, whether it is a person or an agent, and a binding policy naming the
host kind, the capabilities it supplies and its independence group. It is not an
agent. The same design run twice may be played by different ones, and a design
that named a concrete executor would be a deployment pretending to be a
description.

A system declares a kind for each role. The kinds are about authority, not job
titles: who drives the work, who does it, who reads it critically, who signs it
off and who only watches.

| Role kind | Responsibility |
|:--|:--|
| Integrator | Drives the system and answers for its joint result. Only this kind may be named as the system's integrator. |
| Contributor | Produces an assigned result inside a composed ceremony and supplies evidence for it. |
| Reviewer | Reads a result critically. An independence rule is stated between a reviewer role and the role it reviews. |
| Approver | Answers a human guard. Its decision is recorded, never inferred from completed work. |
| Observer | Sees the work without acting on it. |

Validation reads these kinds: a system whose integrator is declared as another
kind, or whom no participant plays, is refused, and an independence rule is
checked participant by participant inside each composed ceremony. That is
analysis before publication, not runtime privilege. The ceremony definition's
allowed actions and guards remain the source of authorization when the work
actually runs.
