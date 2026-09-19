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
assignment or special runtime privilege. Designer guidance and visibility of
the role in live agent interfaces are tracked separately; naming the role
alone does not implement those capabilities.
