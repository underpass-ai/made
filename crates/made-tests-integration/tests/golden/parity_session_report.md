# Parity review &lt;both arms&gt;

Ceremonies: 2 · completed: 1 · incomplete: 1

## Ceremony `parity-session`

- Definition: `parity_session`
- Version: `1.0`
- Definition digest: `0a2031bef3889e91441f668c9780eebc039789355d799c2b0ff75d7328b8b531`
- Bound published digest: not bound
- State: `DONE`
- Status: `completed`
- Created at: `2026-09-16 9:00:00.0 +00:00:00`
- Updated at: `2026-09-16 9:00:00.0 +00:00:00`
- Completed at: `2026-09-16 9:00:00.0 +00:00:00`

### Definition

```json
{
  "name": "parity_session",
  "version": "1.0",
  "description": null,
  "inputs": {},
  "outputs": {},
  "states": {
    "DONE": {
      "id": "DONE",
      "kind": "terminal"
    },
    "OPEN": {
      "id": "OPEN",
      "kind": "initial"
    },
    "REVIEW": {
      "id": "REVIEW",
      "kind": "intermediate"
    }
  },
  "transitions": [
    {
      "from": "OPEN",
      "to": "REVIEW",
      "trigger": "opened",
      "required_guards": [
        "work_done"
      ]
    },
    {
      "from": "REVIEW",
      "to": "DONE",
      "trigger": "approve",
      "required_guards": [
        "human_approved"
      ]
    }
  ],
  "steps": {
    "handoff": {
      "id": "handoff",
      "state_id": "REVIEW",
      "handler_kind": "parity_step",
      "handler_config": {},
      "retry_policy": {
        "max_attempts": 1,
        "backoff": 0
      },
      "timeout": null
    },
    "work": {
      "id": "work",
      "state_id": "OPEN",
      "handler_kind": "parity_step",
      "handler_config": {},
      "retry_policy": {
        "max_attempts": 1,
        "backoff": 0
      },
      "timeout": null
    }
  },
  "step_order": [
    "work",
    "handoff"
  ],
  "guards": {
    "human_approved": {
      "name": "human_approved",
      "condition": {
        "kind": "human_approval"
      }
    },
    "work_done": {
      "name": "work_done",
      "condition": {
        "kind": "step_status",
        "step_id": "work",
        "status": "COMPLETED"
      }
    }
  },
  "roles": {
    "FACILITATOR": {
      "id": "FACILITATOR",
      "allowed_actions": [
        {
          "kind": "step",
          "value": "handoff"
        },
        {
          "kind": "step",
          "value": "work"
        },
        {
          "kind": "transition",
          "value": "approve"
        },
        {
          "kind": "transition",
          "value": "opened"
        },
        {
          "kind": "request_intervention"
        },
        {
          "kind": "respond_to_intervention"
        }
      ]
    },
    "OBSERVER": {
      "id": "OBSERVER",
      "allowed_actions": [
        {
          "kind": "respond_to_intervention"
        }
      ]
    }
  }
}
```


### Steps and outputs

```json
{
  "handoff": {
    "status": "COMPLETED",
    "iteration": 1,
    "attempt": 1,
    "lease": null,
    "output": {
      "attachments": 2,
      "handoff_note": "the reviewer has it"
    },
    "error_message": null
  },
  "work": {
    "status": "COMPLETED",
    "iteration": 1,
    "attempt": 1,
    "lease": null,
    "output": {
      "findings": [
        {
          "about": "work",
          "verdict": "done"
        }
      ],
      "handler": "parity",
      "state": "OPEN",
      "step": "work",
      "summary": "`work` ran in `OPEN`.",
      "winner_content": "The parity handler finished `work`."
    },
    "error_message": null
  }
}
```


### Transitions

```json
[
  {
    "trigger": "opened",
    "from_state": "OPEN",
    "to_state": "REVIEW",
    "applied_by": "FACILITATOR",
    "applied_at": "2026-09-16T09:00:00Z"
  },
  {
    "trigger": "approve",
    "from_state": "REVIEW",
    "to_state": "DONE",
    "applied_by": "FACILITATOR",
    "applied_at": "2026-09-16T09:00:00Z"
  }
]
```


### Guard approvals

```json
[
  {
    "guard_name": "human_approved",
    "approved_by": "FACILITATOR",
    "approved_by_kind": "human",
    "approved_at": "2026-09-16T09:00:00Z"
  }
]
```


### Guard deferrals

```json
[
  {
    "guard_name": "human_approved",
    "deferred_by": "FACILITATOR",
    "deferred_by_kind": "human",
    "content": {
      "statement": "Not yet.",
      "reason": "The reviewer is out.",
      "reconsider_when": [
        "the reviewer is back"
      ]
    },
    "deferred_at": "2026-09-16T09:00:00Z"
  }
]
```


### Interventions and evidence

```json
[
  {
    "id": "what-happened",
    "kind": "opinion",
    "requested_by": "FACILITATOR",
    "target": {
      "kind": "table"
    },
    "request": {
      "message": "What did you see?",
      "details": {
        "asked_at_state": "REVIEW",
        "attempt": 1,
        "severity": 1
      }
    },
    "provenance": null,
    "responses": [
      {
        "role_id": "OBSERVER",
        "content": {
          "message": "The queue was backing up.",
          "details": {
            "confidence": 1,
            "observed": [
              "queue_depth",
              "error_rate"
            ],
            "samples": 12
          }
        },
        "evidence_pack": null,
        "responded_at": "2026-09-16T09:00:00Z"
      }
    ],
    "status": "closed",
    "created_at": "2026-09-16T09:00:00Z",
    "updated_at": "2026-09-16T09:00:00Z",
    "closed_at": "2026-09-16T09:00:00Z"
  },
  {
    "id": "inspect-metrics",
    "kind": "investigation",
    "requested_by": "FACILITATOR",
    "target": {
      "kind": "roles",
      "role_ids": [
        "OBSERVER"
      ]
    },
    "request": {
      "message": "Inspect the checkout metrics.",
      "details": {}
    },
    "provenance": null,
    "responses": [
      {
        "role_id": "OBSERVER",
        "content": {
          "message": "The source answered the investigation.",
          "details": {
            "evidence_pack": {
              "bundle": {
                "bundle_id": "observability",
                "items": [
                  {
                    "attributes": {},
                    "item_id": "parity-observation",
                    "kind": "metric",
                    "narrative": "Asked `Checkout errors over the last five minutes.`; the answer is 18%.",
                    "reference_ids": [],
                    "title": "What the source found"
                  }
                ],
                "metadata": {},
                "references": [],
                "schema_version": "1.0",
                "summary": {
                  "attributes": {},
                  "text": "The source answered the investigation."
                }
              },
              "collected_at": "2026-09-16T09:00:00Z",
              "source_id": "observability"
            }
          }
        },
        "evidence_pack": {
          "source_id": "observability",
          "bundle": {
            "bundle_id": "observability",
            "schema_version": "1.0",
            "summary": {
              "text": "The source answered the investigation.",
              "attributes": {}
            },
            "items": [
              {
                "item_id": "parity-observation",
                "kind": "metric",
                "title": "What the source found",
                "narrative": "Asked `Checkout errors over the last five minutes.`; the answer is 18%.",
                "attributes": {},
                "reference_ids": []
              }
            ],
            "references": [],
            "metadata": {}
          },
          "collected_at": "2026-09-16T09:00:00Z"
        },
        "responded_at": "2026-09-16T09:00:00Z"
      }
    ],
    "status": "open",
    "created_at": "2026-09-16T09:00:00Z",
    "updated_at": "2026-09-16T09:00:00Z",
    "closed_at": null
  }
]
```


### Reasons

```json
[
  {
    "from": {
      "kind": "contribution",
      "agenda_item": "what-happened",
      "ordinal": 0
    },
    "to": {
      "kind": "agenda_item",
      "agenda_item": "what-happened"
    },
    "kind": "answers",
    "why": "a contribution made against this agenda item",
    "confidence": "high",
    "asserted_by": null,
    "asserted_at": "2026-09-16T09:00:00Z"
  },
  {
    "from": {
      "kind": "contribution",
      "agenda_item": "inspect-metrics",
      "ordinal": 0
    },
    "to": {
      "kind": "agenda_item",
      "agenda_item": "inspect-metrics"
    },
    "kind": "answers",
    "why": "a contribution made against this agenda item",
    "confidence": "high",
    "asserted_by": null,
    "asserted_at": "2026-09-16T09:00:00Z"
  },
  {
    "from": {
      "kind": "contribution",
      "agenda_item": "inspect-metrics",
      "ordinal": 0
    },
    "to": {
      "kind": "contribution",
      "agenda_item": "what-happened",
      "ordinal": 0
    },
    "kind": "chosen_because",
    "why": "The queue growth is what sent me to the metrics.",
    "confidence": "high",
    "asserted_by": "OBSERVER",
    "asserted_at": "2026-09-16T09:00:00Z"
  }
]
```


### Audit journal

```json
[
  {
    "event_id": "parity-session:ceremony_instance_started:session",
    "event_type": "ceremony_instance_started",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 1,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "parity-operator",
      "kind": "service",
      "role_id": null
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": null,
    "trace_id": "<trace-id>",
    "event_schema_version": 1,
    "event": {
      "type": "ceremony_instance_started",
      "ceremony_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "initial_state": "OPEN",
      "step_ids": [
        "handoff",
        "work"
      ],
      "context": {
        "incident_ref": "INC-42",
        "severity": 2
      },
      "bound_definition": null,
      "created_at": "2026-09-16T09:00:00Z"
    },
    "previous_record_hash": null,
    "record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  },
  {
    "event_id": "parity-session:participants_bound:seating:FACILITATOR=facilitation",
    "event_type": "participants_bound",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 2,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "parity-operator",
      "kind": "service",
      "role_id": null
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:ceremony_instance_started:session",
    "trace_id": "<trace-id>",
    "event_schema_version": 1,
    "event": {
      "type": "participants_bound",
      "bindings": [
        {
          "role_id": "FACILITATOR",
          "specialty": "facilitation",
          "bound_at": "2026-09-16T09:00:00Z"
        }
      ]
    },
    "previous_record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  },
  {
    "event_id": "parity-session:step_started:step:work:iteration:1:attempt:1",
    "event_type": "step_started",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 3,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "FACILITATOR",
      "kind": "agent",
      "role_id": "FACILITATOR"
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:participants_bound:seating:FACILITATOR=facilitation",
    "trace_id": "<trace-id>",
    "event_schema_version": 1,
    "event": {
      "type": "step_started",
      "step_id": "work",
      "iteration": 1,
      "attempt": 1,
      "lease": {
        "owner_id": "parity-host",
        "idempotency_key": "parity-work-1",
        "acquired_at": "2026-09-16T09:00:00Z",
        "expires_at": "2026-09-16T09:00:30Z"
      },
      "started_by": "FACILITATOR",
      "started_at": "2026-09-16T09:00:00Z"
    },
    "previous_record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  },
  {
    "event_id": "parity-session:step_completed:step:work:iteration:1:attempt:1",
    "event_type": "step_completed",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 4,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "FACILITATOR",
      "kind": "agent",
      "role_id": "FACILITATOR"
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:step_started:step:work:iteration:1:attempt:1",
    "trace_id": "<trace-id>",
    "event_schema_version": 1,
    "event": {
      "type": "step_completed",
      "step_id": "work",
      "iteration": 1,
      "attempt": 1,
      "result": {
        "status": "COMPLETED",
        "output": {
          "findings": [
            {
              "about": "work",
              "verdict": "done"
            }
          ],
          "handler": "parity",
          "state": "OPEN",
          "step": "work",
          "summary": "`work` ran in `OPEN`.",
          "winner_content": "The parity handler finished `work`."
        },
        "error_message": null
      },
      "next_iteration": null,
      "finished_by": "FACILITATOR",
      "finished_at": "2026-09-16T09:00:00Z"
    },
    "previous_record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  },
  {
    "event_id": "parity-session:transition_applied:transition:1",
    "event_type": "transition_applied",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 5,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "FACILITATOR",
      "kind": "agent",
      "role_id": "FACILITATOR"
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:step_completed:step:work:iteration:1:attempt:1",
    "trace_id": "<trace-id>",
    "event_schema_version": 1,
    "event": {
      "type": "transition_applied",
      "transition": {
        "trigger": "opened",
        "from_state": "OPEN",
        "to_state": "REVIEW",
        "applied_by": "FACILITATOR",
        "applied_at": "2026-09-16T09:00:00Z"
      }
    },
    "previous_record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  },
  {
    "event_id": "parity-session:step_started:step:handoff:iteration:1:attempt:1",
    "event_type": "step_started",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 6,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "FACILITATOR",
      "kind": "agent",
      "role_id": "FACILITATOR"
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:transition_applied:transition:1",
    "trace_id": "<trace-id>",
    "event_schema_version": 1,
    "event": {
      "type": "step_started",
      "step_id": "handoff",
      "iteration": 1,
      "attempt": 1,
      "lease": {
        "owner_id": "parity-host",
        "idempotency_key": "parity-handoff-1",
        "acquired_at": "2026-09-16T09:00:00Z",
        "expires_at": "2026-09-16T09:01:00Z"
      },
      "started_by": "FACILITATOR",
      "started_at": "2026-09-16T09:00:00Z"
    },
    "previous_record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  },
  {
    "event_id": "parity-session:step_completed:step:handoff:iteration:1:attempt:1",
    "event_type": "step_completed",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 7,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "FACILITATOR",
      "kind": "agent",
      "role_id": "FACILITATOR"
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:step_started:step:handoff:iteration:1:attempt:1",
    "trace_id": "<trace-id>",
    "event_schema_version": 1,
    "event": {
      "type": "step_completed",
      "step_id": "handoff",
      "iteration": 1,
      "attempt": 1,
      "result": {
        "status": "COMPLETED",
        "output": {
          "attachments": 2,
          "handoff_note": "the reviewer has it"
        },
        "error_message": null
      },
      "next_iteration": null,
      "finished_by": "FACILITATOR",
      "finished_at": "2026-09-16T09:00:00Z"
    },
    "previous_record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  },
  {
    "event_id": "parity-session:intervention_requested:intervention:what-happened",
    "event_type": "intervention_requested",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 8,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "FACILITATOR",
      "kind": "human",
      "role_id": "FACILITATOR"
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:step_completed:step:handoff:iteration:1:attempt:1",
    "trace_id": "<trace-id>",
    "event_schema_version": 1,
    "event": {
      "type": "intervention_requested",
      "intervention": {
        "id": "what-happened",
        "kind": "opinion",
        "requested_by": "FACILITATOR",
        "target": {
          "kind": "table"
        },
        "request": {
          "message": "What did you see?",
          "details": {
            "asked_at_state": "REVIEW",
            "attempt": 1,
            "severity": 1
          }
        },
        "provenance": null,
        "responses": [],
        "status": "open",
        "created_at": "2026-09-16T09:00:00Z",
        "updated_at": "2026-09-16T09:00:00Z",
        "closed_at": null
      }
    },
    "previous_record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  },
  {
    "event_id": "parity-session:intervention_responded:intervention:what-happened:OBSERVER",
    "event_type": "intervention_responded",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 9,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "OBSERVER",
      "kind": "agent",
      "role_id": "OBSERVER"
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:intervention_requested:intervention:what-happened",
    "trace_id": "<trace-id>",
    "event_schema_version": 1,
    "event": {
      "type": "intervention_responded",
      "intervention_id": "what-happened",
      "response": {
        "role_id": "OBSERVER",
        "content": {
          "message": "The queue was backing up.",
          "details": {
            "confidence": 1,
            "observed": [
              "queue_depth",
              "error_rate"
            ],
            "samples": 12
          }
        },
        "evidence_pack": null,
        "responded_at": "2026-09-16T09:00:00Z"
      }
    },
    "previous_record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  },
  {
    "event_id": "parity-session:intervention_requested:intervention:inspect-metrics",
    "event_type": "intervention_requested",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 10,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "FACILITATOR",
      "kind": "human",
      "role_id": "FACILITATOR"
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:intervention_responded:intervention:what-happened:OBSERVER",
    "trace_id": "<trace-id>",
    "event_schema_version": 1,
    "event": {
      "type": "intervention_requested",
      "intervention": {
        "id": "inspect-metrics",
        "kind": "investigation",
        "requested_by": "FACILITATOR",
        "target": {
          "kind": "roles",
          "role_ids": [
            "OBSERVER"
          ]
        },
        "request": {
          "message": "Inspect the checkout metrics.",
          "details": {}
        },
        "provenance": null,
        "responses": [],
        "status": "open",
        "created_at": "2026-09-16T09:00:00Z",
        "updated_at": "2026-09-16T09:00:00Z",
        "closed_at": null
      }
    },
    "previous_record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  },
  {
    "event_id": "parity-session:evidence_collected:intervention:inspect-metrics:source:observability",
    "event_type": "evidence_collected",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 11,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "OBSERVER",
      "kind": "agent",
      "role_id": "OBSERVER"
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:intervention_requested:intervention:inspect-metrics",
    "trace_id": "<trace-id>",
    "event_schema_version": 1,
    "event": {
      "type": "evidence_collected",
      "intervention_id": "inspect-metrics",
      "source_id": "observability",
      "collected_by": "OBSERVER",
      "evidence_pack": {
        "source_id": "observability",
        "bundle": {
          "bundle_id": "observability",
          "schema_version": "1.0",
          "summary": {
            "text": "The source answered the investigation.",
            "attributes": {}
          },
          "items": [
            {
              "item_id": "parity-observation",
              "kind": "metric",
              "title": "What the source found",
              "narrative": "Asked `Checkout errors over the last five minutes.`; the answer is 18%.",
              "attributes": {},
              "reference_ids": []
            }
          ],
          "references": [],
          "metadata": {}
        },
        "collected_at": "2026-09-16T09:00:00Z"
      },
      "collected_at": "2026-09-16T09:00:00Z"
    },
    "previous_record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  },
  {
    "event_id": "parity-session:intervention_responded:intervention:inspect-metrics:OBSERVER",
    "event_type": "intervention_responded",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 12,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "OBSERVER",
      "kind": "agent",
      "role_id": "OBSERVER"
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:evidence_collected:intervention:inspect-metrics:source:observability",
    "trace_id": "<trace-id>",
    "event_schema_version": 1,
    "event": {
      "type": "intervention_responded",
      "intervention_id": "inspect-metrics",
      "response": {
        "role_id": "OBSERVER",
        "content": {
          "message": "The source answered the investigation.",
          "details": {
            "evidence_pack": {
              "bundle": {
                "bundle_id": "observability",
                "items": [
                  {
                    "attributes": {},
                    "item_id": "parity-observation",
                    "kind": "metric",
                    "narrative": "Asked `Checkout errors over the last five minutes.`; the answer is 18%.",
                    "reference_ids": [],
                    "title": "What the source found"
                  }
                ],
                "metadata": {},
                "references": [],
                "schema_version": "1.0",
                "summary": {
                  "attributes": {},
                  "text": "The source answered the investigation."
                }
              },
              "collected_at": "2026-09-16T09:00:00Z",
              "source_id": "observability"
            }
          }
        },
        "evidence_pack": {
          "source_id": "observability",
          "bundle": {
            "bundle_id": "observability",
            "schema_version": "1.0",
            "summary": {
              "text": "The source answered the investigation.",
              "attributes": {}
            },
            "items": [
              {
                "item_id": "parity-observation",
                "kind": "metric",
                "title": "What the source found",
                "narrative": "Asked `Checkout errors over the last five minutes.`; the answer is 18%.",
                "attributes": {},
                "reference_ids": []
              }
            ],
            "references": [],
            "metadata": {}
          },
          "collected_at": "2026-09-16T09:00:00Z"
        },
        "responded_at": "2026-09-16T09:00:00Z"
      }
    },
    "previous_record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  },
  {
    "event_id": "parity-session:reason_asserted:reason:3",
    "event_type": "reason_asserted",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 13,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "OBSERVER",
      "kind": "agent",
      "role_id": "OBSERVER"
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:intervention_responded:intervention:inspect-metrics:OBSERVER",
    "trace_id": "<trace-id>",
    "event_schema_version": 1,
    "event": {
      "type": "reason_asserted",
      "reason": {
        "from": {
          "kind": "contribution",
          "agenda_item": "inspect-metrics",
          "ordinal": 0
        },
        "to": {
          "kind": "contribution",
          "agenda_item": "what-happened",
          "ordinal": 0
        },
        "kind": "chosen_because",
        "why": "The queue growth is what sent me to the metrics.",
        "confidence": "high",
        "asserted_by": "OBSERVER",
        "asserted_at": "2026-09-16T09:00:00Z"
      }
    },
    "previous_record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  },
  {
    "event_id": "parity-session:intervention_closed:intervention:what-happened",
    "event_type": "intervention_closed",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 14,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "FACILITATOR",
      "kind": "human",
      "role_id": "FACILITATOR"
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:reason_asserted:reason:3",
    "trace_id": "<trace-id>",
    "event_schema_version": 1,
    "event": {
      "type": "intervention_closed",
      "intervention_id": "what-happened",
      "closed_by": "FACILITATOR",
      "closed_at": "2026-09-16T09:00:00Z"
    },
    "previous_record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  },
  {
    "event_id": "parity-session:human_deferral_recorded:guard:human_approved",
    "event_type": "human_deferral_recorded",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 15,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "FACILITATOR",
      "kind": "human",
      "role_id": "FACILITATOR"
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:intervention_closed:intervention:what-happened",
    "trace_id": "<trace-id>",
    "event_schema_version": 1,
    "event": {
      "type": "human_deferral_recorded",
      "deferral": {
        "guard_name": "human_approved",
        "deferred_by": "FACILITATOR",
        "deferred_by_kind": "human",
        "content": {
          "statement": "Not yet.",
          "reason": "The reviewer is out.",
          "reconsider_when": [
            "the reviewer is back"
          ]
        },
        "deferred_at": "2026-09-16T09:00:00Z"
      }
    },
    "previous_record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  },
  {
    "event_id": "parity-session:human_approval_recorded:guard:human_approved",
    "event_type": "human_approval_recorded",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 16,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "FACILITATOR",
      "kind": "human",
      "role_id": "FACILITATOR"
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:human_deferral_recorded:guard:human_approved",
    "trace_id": "<trace-id>",
    "event_schema_version": 1,
    "event": {
      "type": "human_approval_recorded",
      "approval": {
        "guard_name": "human_approved",
        "approved_by": "FACILITATOR",
        "approved_by_kind": "human",
        "approved_at": "2026-09-16T09:00:00Z"
      }
    },
    "previous_record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  },
  {
    "event_id": "parity-session:transition_applied:transition:2",
    "event_type": "transition_applied",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 17,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "FACILITATOR",
      "kind": "human",
      "role_id": "FACILITATOR"
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:human_approval_recorded:guard:human_approved",
    "trace_id": "<trace-id>",
    "event_schema_version": 1,
    "event": {
      "type": "transition_applied",
      "transition": {
        "trigger": "approve",
        "from_state": "REVIEW",
        "to_state": "DONE",
        "applied_by": "FACILITATOR",
        "applied_at": "2026-09-16T09:00:00Z"
      }
    },
    "previous_record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  },
  {
    "event_id": "parity-session:ceremony_completed:transition:2",
    "event_type": "ceremony_completed",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 18,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "FACILITATOR",
      "kind": "human",
      "role_id": "FACILITATOR"
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:transition_applied:transition:2",
    "trace_id": "<trace-id>",
    "event_schema_version": 1,
    "event": {
      "type": "ceremony_completed",
      "final_state": "DONE",
      "completed_at": "2026-09-16T09:00:00Z"
    },
    "previous_record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  }
]
```


## Ceremony `parity-published-session`

- Definition: `parity_published`
- Version: `1.0`
- Definition digest: `e137a8cadec3279c0b0e1a5c0287d7473ed12b6e8464161804541809c20f2bbe`
- Bound published digest: `e137a8cadec3279c0b0e1a5c0287d7473ed12b6e8464161804541809c20f2bbe`
- State: `OPEN`
- Status: `incomplete`
- Created at: `2026-09-16 9:00:00.0 +00:00:00`
- Updated at: `2026-09-16 9:00:00.0 +00:00:00`
- Completed at: not available

### Definition

```json
{
  "name": "parity_published",
  "version": "1.0",
  "description": null,
  "inputs": {},
  "outputs": {},
  "states": {
    "DONE": {
      "id": "DONE",
      "kind": "terminal"
    },
    "OPEN": {
      "id": "OPEN",
      "kind": "initial"
    }
  },
  "transitions": [
    {
      "from": "OPEN",
      "to": "DONE",
      "trigger": "finish",
      "required_guards": []
    }
  ],
  "steps": {
    "work": {
      "id": "work",
      "state_id": "OPEN",
      "handler_kind": "parity_step",
      "handler_config": {},
      "retry_policy": {
        "max_attempts": 1,
        "backoff": 0
      },
      "timeout": null
    }
  },
  "step_order": [
    "work"
  ],
  "guards": {},
  "roles": {
    "FACILITATOR": {
      "id": "FACILITATOR",
      "allowed_actions": [
        {
          "kind": "step",
          "value": "work"
        },
        {
          "kind": "transition",
          "value": "finish"
        }
      ]
    }
  }
}
```


### Steps and outputs

```json
{
  "work": {
    "status": "PENDING",
    "iteration": 1,
    "attempt": 1,
    "lease": null,
    "output": {},
    "error_message": null
  }
}
```


### Transitions

```json
[]
```


### Guard approvals

```json
[]
```


### Guard deferrals

```json
[]
```


### Interventions and evidence

```json
[]
```


### Reasons

```json
[]
```


### Audit journal

```json
[
  {
    "event_id": "parity-published-session:ceremony_instance_started:session",
    "event_type": "ceremony_instance_started",
    "schema_version": 2,
    "ceremony_id": "parity-published-session",
    "definition_name": "parity_published",
    "definition_version": "1.0",
    "sequence": 1,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "parity-operator",
      "kind": "service",
      "role_id": null
    },
    "correlation_id": "parity-published-session:ceremony_instance_started:session",
    "causation_id": null,
    "trace_id": "<trace-id>",
    "event_schema_version": 1,
    "event": {
      "type": "ceremony_instance_started",
      "ceremony_id": "parity-published-session",
      "definition_name": "parity_published",
      "definition_version": "1.0",
      "initial_state": "OPEN",
      "step_ids": [
        "work"
      ],
      "context": {},
      "bound_definition": [
        225,
        55,
        168,
        202,
        222,
        195,
        39,
        156,
        11,
        14,
        26,
        92,
        2,
        135,
        215,
        71,
        62,
        209,
        43,
        110,
        132,
        100,
        22,
        24,
        4,
        84,
        24,
        9,
        194,
        15,
        43,
        190
      ],
      "created_at": "2026-09-16T09:00:00Z"
    },
    "previous_record_hash": null,
    "record_hash": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  }
]
```
