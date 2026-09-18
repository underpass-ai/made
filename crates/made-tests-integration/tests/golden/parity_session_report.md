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
    "state_iteration": 1,
    "to_state": "REVIEW",
    "applied_by": "FACILITATOR",
    "applied_at": "2026-09-16T09:00:00Z"
  },
  {
    "trigger": "approve",
    "from_state": "REVIEW",
    "state_iteration": 1,
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
    "status": "closed",
    "created_at": "2026-09-16T09:00:00Z",
    "updated_at": "2026-09-16T09:00:00Z",
    "closed_at": "2026-09-16T09:00:00Z"
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
    "trace_id": "00000000000000000000000000000007",
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
    "record_hash": [
      204,
      134,
      45,
      202,
      208,
      161,
      160,
      62,
      23,
      219,
      122,
      7,
      227,
      247,
      1,
      2,
      155,
      223,
      53,
      217,
      237,
      38,
      70,
      93,
      242,
      24,
      146,
      203,
      161,
      175,
      186,
      224
    ]
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
    "trace_id": "00000000000000000000000000000008",
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
    "previous_record_hash": [
      204,
      134,
      45,
      202,
      208,
      161,
      160,
      62,
      23,
      219,
      122,
      7,
      227,
      247,
      1,
      2,
      155,
      223,
      53,
      217,
      237,
      38,
      70,
      93,
      242,
      24,
      146,
      203,
      161,
      175,
      186,
      224
    ],
    "record_hash": [
      37,
      201,
      115,
      120,
      54,
      71,
      62,
      98,
      93,
      179,
      130,
      73,
      198,
      40,
      178,
      94,
      2,
      171,
      116,
      18,
      196,
      212,
      45,
      218,
      80,
      175,
      97,
      5,
      6,
      90,
      138,
      109
    ]
  },
  {
    "event_id": "parity-session:step_started:step:work:state_iteration:1:iteration:1:attempt:1",
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
    "trace_id": "00000000000000000000000000000009",
    "event_schema_version": 2,
    "event": {
      "type": "step_started",
      "step_id": "work",
      "state_iteration": 1,
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
    "previous_record_hash": [
      37,
      201,
      115,
      120,
      54,
      71,
      62,
      98,
      93,
      179,
      130,
      73,
      198,
      40,
      178,
      94,
      2,
      171,
      116,
      18,
      196,
      212,
      45,
      218,
      80,
      175,
      97,
      5,
      6,
      90,
      138,
      109
    ],
    "record_hash": [
      104,
      32,
      29,
      69,
      152,
      234,
      11,
      149,
      59,
      150,
      180,
      128,
      190,
      38,
      192,
      235,
      163,
      29,
      111,
      208,
      123,
      24,
      16,
      98,
      35,
      255,
      122,
      122,
      72,
      69,
      173,
      82
    ]
  },
  {
    "event_id": "parity-session:step_completed:step:work:state_iteration:1:iteration:1:attempt:1",
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
    "causation_id": "parity-session:step_started:step:work:state_iteration:1:iteration:1:attempt:1",
    "trace_id": "00000000000000000000000000000009",
    "event_schema_version": 2,
    "event": {
      "type": "step_completed",
      "step_id": "work",
      "state_iteration": 1,
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
    "previous_record_hash": [
      104,
      32,
      29,
      69,
      152,
      234,
      11,
      149,
      59,
      150,
      180,
      128,
      190,
      38,
      192,
      235,
      163,
      29,
      111,
      208,
      123,
      24,
      16,
      98,
      35,
      255,
      122,
      122,
      72,
      69,
      173,
      82
    ],
    "record_hash": [
      108,
      63,
      111,
      48,
      216,
      194,
      5,
      196,
      252,
      132,
      141,
      252,
      130,
      18,
      74,
      56,
      170,
      240,
      89,
      217,
      164,
      84,
      20,
      215,
      120,
      28,
      244,
      81,
      185,
      246,
      224,
      39
    ]
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
    "causation_id": "parity-session:step_completed:step:work:state_iteration:1:iteration:1:attempt:1",
    "trace_id": "0000000000000000000000000000000a",
    "event_schema_version": 2,
    "event": {
      "type": "transition_applied",
      "transition": {
        "trigger": "opened",
        "from_state": "OPEN",
        "state_iteration": 1,
        "to_state": "REVIEW",
        "applied_by": "FACILITATOR",
        "applied_at": "2026-09-16T09:00:00Z"
      }
    },
    "previous_record_hash": [
      108,
      63,
      111,
      48,
      216,
      194,
      5,
      196,
      252,
      132,
      141,
      252,
      130,
      18,
      74,
      56,
      170,
      240,
      89,
      217,
      164,
      84,
      20,
      215,
      120,
      28,
      244,
      81,
      185,
      246,
      224,
      39
    ],
    "record_hash": [
      245,
      233,
      157,
      13,
      179,
      228,
      50,
      218,
      115,
      106,
      103,
      201,
      191,
      166,
      214,
      92,
      203,
      214,
      48,
      171,
      113,
      19,
      32,
      13,
      214,
      156,
      28,
      6,
      73,
      44,
      142,
      24
    ]
  },
  {
    "event_id": "parity-session:step_started:step:handoff:state_iteration:1:iteration:1:attempt:1",
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
    "trace_id": "0000000000000000000000000000000b",
    "event_schema_version": 2,
    "event": {
      "type": "step_started",
      "step_id": "handoff",
      "state_iteration": 1,
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
    "previous_record_hash": [
      245,
      233,
      157,
      13,
      179,
      228,
      50,
      218,
      115,
      106,
      103,
      201,
      191,
      166,
      214,
      92,
      203,
      214,
      48,
      171,
      113,
      19,
      32,
      13,
      214,
      156,
      28,
      6,
      73,
      44,
      142,
      24
    ],
    "record_hash": [
      130,
      88,
      228,
      19,
      155,
      90,
      195,
      200,
      131,
      69,
      87,
      153,
      129,
      131,
      146,
      171,
      211,
      187,
      3,
      99,
      180,
      123,
      48,
      32,
      124,
      41,
      252,
      255,
      54,
      171,
      163,
      113
    ]
  },
  {
    "event_id": "parity-session:step_completed:step:handoff:state_iteration:1:iteration:1:attempt:1",
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
    "causation_id": "parity-session:step_started:step:handoff:state_iteration:1:iteration:1:attempt:1",
    "trace_id": "0000000000000000000000000000000c",
    "event_schema_version": 2,
    "event": {
      "type": "step_completed",
      "step_id": "handoff",
      "state_iteration": 1,
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
    "previous_record_hash": [
      130,
      88,
      228,
      19,
      155,
      90,
      195,
      200,
      131,
      69,
      87,
      153,
      129,
      131,
      146,
      171,
      211,
      187,
      3,
      99,
      180,
      123,
      48,
      32,
      124,
      41,
      252,
      255,
      54,
      171,
      163,
      113
    ],
    "record_hash": [
      234,
      131,
      62,
      107,
      225,
      57,
      44,
      152,
      177,
      112,
      103,
      46,
      227,
      114,
      193,
      52,
      210,
      251,
      213,
      185,
      252,
      215,
      73,
      45,
      179,
      89,
      180,
      245,
      40,
      219,
      224,
      155
    ]
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
    "causation_id": "parity-session:step_completed:step:handoff:state_iteration:1:iteration:1:attempt:1",
    "trace_id": "0000000000000000000000000000000d",
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
    "previous_record_hash": [
      234,
      131,
      62,
      107,
      225,
      57,
      44,
      152,
      177,
      112,
      103,
      46,
      227,
      114,
      193,
      52,
      210,
      251,
      213,
      185,
      252,
      215,
      73,
      45,
      179,
      89,
      180,
      245,
      40,
      219,
      224,
      155
    ],
    "record_hash": [
      172,
      42,
      113,
      200,
      187,
      17,
      23,
      52,
      196,
      186,
      151,
      223,
      80,
      220,
      10,
      48,
      82,
      187,
      142,
      159,
      1,
      76,
      157,
      162,
      9,
      230,
      45,
      215,
      138,
      128,
      22,
      26
    ]
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
    "trace_id": "0000000000000000000000000000000e",
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
    "previous_record_hash": [
      172,
      42,
      113,
      200,
      187,
      17,
      23,
      52,
      196,
      186,
      151,
      223,
      80,
      220,
      10,
      48,
      82,
      187,
      142,
      159,
      1,
      76,
      157,
      162,
      9,
      230,
      45,
      215,
      138,
      128,
      22,
      26
    ],
    "record_hash": [
      98,
      184,
      204,
      157,
      20,
      130,
      182,
      231,
      44,
      33,
      250,
      0,
      153,
      211,
      62,
      159,
      83,
      229,
      4,
      213,
      218,
      80,
      52,
      39,
      116,
      75,
      231,
      243,
      132,
      239,
      70,
      49
    ]
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
    "trace_id": "0000000000000000000000000000000f",
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
    "previous_record_hash": [
      98,
      184,
      204,
      157,
      20,
      130,
      182,
      231,
      44,
      33,
      250,
      0,
      153,
      211,
      62,
      159,
      83,
      229,
      4,
      213,
      218,
      80,
      52,
      39,
      116,
      75,
      231,
      243,
      132,
      239,
      70,
      49
    ],
    "record_hash": [
      28,
      153,
      195,
      10,
      218,
      163,
      158,
      115,
      224,
      59,
      198,
      19,
      215,
      173,
      40,
      198,
      195,
      117,
      19,
      192,
      249,
      203,
      223,
      32,
      51,
      98,
      109,
      227,
      63,
      222,
      14,
      128
    ]
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
    "trace_id": "00000000000000000000000000000010",
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
    "previous_record_hash": [
      28,
      153,
      195,
      10,
      218,
      163,
      158,
      115,
      224,
      59,
      198,
      19,
      215,
      173,
      40,
      198,
      195,
      117,
      19,
      192,
      249,
      203,
      223,
      32,
      51,
      98,
      109,
      227,
      63,
      222,
      14,
      128
    ],
    "record_hash": [
      194,
      255,
      153,
      231,
      226,
      138,
      39,
      127,
      15,
      198,
      134,
      77,
      65,
      121,
      75,
      63,
      186,
      131,
      63,
      109,
      42,
      166,
      94,
      58,
      114,
      77,
      89,
      181,
      87,
      50,
      50,
      136
    ]
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
    "trace_id": "00000000000000000000000000000010",
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
    "previous_record_hash": [
      194,
      255,
      153,
      231,
      226,
      138,
      39,
      127,
      15,
      198,
      134,
      77,
      65,
      121,
      75,
      63,
      186,
      131,
      63,
      109,
      42,
      166,
      94,
      58,
      114,
      77,
      89,
      181,
      87,
      50,
      50,
      136
    ],
    "record_hash": [
      125,
      136,
      170,
      86,
      155,
      170,
      106,
      155,
      225,
      157,
      55,
      200,
      5,
      152,
      132,
      16,
      208,
      156,
      17,
      212,
      110,
      17,
      129,
      130,
      137,
      98,
      38,
      201,
      218,
      167,
      46,
      185
    ]
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
    "trace_id": "00000000000000000000000000000011",
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
    "previous_record_hash": [
      125,
      136,
      170,
      86,
      155,
      170,
      106,
      155,
      225,
      157,
      55,
      200,
      5,
      152,
      132,
      16,
      208,
      156,
      17,
      212,
      110,
      17,
      129,
      130,
      137,
      98,
      38,
      201,
      218,
      167,
      46,
      185
    ],
    "record_hash": [
      70,
      239,
      226,
      95,
      70,
      88,
      83,
      7,
      37,
      194,
      0,
      68,
      140,
      14,
      235,
      210,
      50,
      124,
      206,
      37,
      172,
      194,
      62,
      254,
      40,
      75,
      146,
      249,
      90,
      55,
      104,
      114
    ]
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
    "trace_id": "00000000000000000000000000000012",
    "event_schema_version": 1,
    "event": {
      "type": "intervention_closed",
      "intervention_id": "what-happened",
      "closed_by": "FACILITATOR",
      "closed_at": "2026-09-16T09:00:00Z"
    },
    "previous_record_hash": [
      70,
      239,
      226,
      95,
      70,
      88,
      83,
      7,
      37,
      194,
      0,
      68,
      140,
      14,
      235,
      210,
      50,
      124,
      206,
      37,
      172,
      194,
      62,
      254,
      40,
      75,
      146,
      249,
      90,
      55,
      104,
      114
    ],
    "record_hash": [
      26,
      64,
      2,
      145,
      45,
      85,
      61,
      72,
      12,
      201,
      24,
      22,
      184,
      228,
      211,
      106,
      93,
      180,
      178,
      9,
      36,
      101,
      16,
      97,
      28,
      101,
      74,
      149,
      158,
      30,
      144,
      99
    ]
  },
  {
    "event_id": "parity-session:intervention_closed:intervention:inspect-metrics",
    "event_type": "intervention_closed",
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
    "trace_id": "00000000000000000000000000000013",
    "event_schema_version": 1,
    "event": {
      "type": "intervention_closed",
      "intervention_id": "inspect-metrics",
      "closed_by": "FACILITATOR",
      "closed_at": "2026-09-16T09:00:00Z"
    },
    "previous_record_hash": [
      26,
      64,
      2,
      145,
      45,
      85,
      61,
      72,
      12,
      201,
      24,
      22,
      184,
      228,
      211,
      106,
      93,
      180,
      178,
      9,
      36,
      101,
      16,
      97,
      28,
      101,
      74,
      149,
      158,
      30,
      144,
      99
    ],
    "record_hash": [
      211,
      25,
      102,
      178,
      176,
      4,
      168,
      198,
      4,
      126,
      60,
      14,
      218,
      45,
      111,
      241,
      136,
      246,
      72,
      159,
      48,
      26,
      180,
      223,
      188,
      142,
      69,
      127,
      38,
      173,
      217,
      228
    ]
  },
  {
    "event_id": "parity-session:human_deferral_recorded:guard:human_approved",
    "event_type": "human_deferral_recorded",
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
    "causation_id": "parity-session:intervention_closed:intervention:inspect-metrics",
    "trace_id": "00000000000000000000000000000014",
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
    "previous_record_hash": [
      211,
      25,
      102,
      178,
      176,
      4,
      168,
      198,
      4,
      126,
      60,
      14,
      218,
      45,
      111,
      241,
      136,
      246,
      72,
      159,
      48,
      26,
      180,
      223,
      188,
      142,
      69,
      127,
      38,
      173,
      217,
      228
    ],
    "record_hash": [
      216,
      232,
      176,
      128,
      141,
      57,
      83,
      222,
      102,
      105,
      142,
      68,
      212,
      143,
      6,
      33,
      109,
      232,
      164,
      144,
      4,
      207,
      69,
      201,
      230,
      193,
      21,
      190,
      172,
      200,
      231,
      195
    ]
  },
  {
    "event_id": "parity-session:human_approval_recorded:guard:human_approved",
    "event_type": "human_approval_recorded",
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
    "causation_id": "parity-session:human_deferral_recorded:guard:human_approved",
    "trace_id": "00000000000000000000000000000015",
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
    "previous_record_hash": [
      216,
      232,
      176,
      128,
      141,
      57,
      83,
      222,
      102,
      105,
      142,
      68,
      212,
      143,
      6,
      33,
      109,
      232,
      164,
      144,
      4,
      207,
      69,
      201,
      230,
      193,
      21,
      190,
      172,
      200,
      231,
      195
    ],
    "record_hash": [
      67,
      152,
      136,
      241,
      81,
      162,
      29,
      218,
      234,
      219,
      99,
      180,
      209,
      227,
      202,
      43,
      251,
      91,
      133,
      201,
      99,
      106,
      217,
      221,
      214,
      126,
      88,
      143,
      82,
      241,
      216,
      172
    ]
  },
  {
    "event_id": "parity-session:transition_applied:transition:2",
    "event_type": "transition_applied",
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
    "causation_id": "parity-session:human_approval_recorded:guard:human_approved",
    "trace_id": "00000000000000000000000000000016",
    "event_schema_version": 2,
    "event": {
      "type": "transition_applied",
      "transition": {
        "trigger": "approve",
        "from_state": "REVIEW",
        "state_iteration": 1,
        "to_state": "DONE",
        "applied_by": "FACILITATOR",
        "applied_at": "2026-09-16T09:00:00Z"
      }
    },
    "previous_record_hash": [
      67,
      152,
      136,
      241,
      81,
      162,
      29,
      218,
      234,
      219,
      99,
      180,
      209,
      227,
      202,
      43,
      251,
      91,
      133,
      201,
      99,
      106,
      217,
      221,
      214,
      126,
      88,
      143,
      82,
      241,
      216,
      172
    ],
    "record_hash": [
      199,
      196,
      139,
      73,
      47,
      39,
      183,
      64,
      101,
      102,
      2,
      211,
      78,
      29,
      217,
      141,
      175,
      15,
      146,
      76,
      181,
      137,
      19,
      5,
      44,
      65,
      11,
      202,
      141,
      60,
      24,
      113
    ]
  },
  {
    "event_id": "parity-session:ceremony_completed:transition:2",
    "event_type": "ceremony_completed",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 19,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "FACILITATOR",
      "kind": "human",
      "role_id": "FACILITATOR"
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:transition_applied:transition:2",
    "trace_id": "00000000000000000000000000000016",
    "event_schema_version": 1,
    "event": {
      "type": "ceremony_completed",
      "final_state": "DONE",
      "completed_at": "2026-09-16T09:00:00Z"
    },
    "previous_record_hash": [
      199,
      196,
      139,
      73,
      47,
      39,
      183,
      64,
      101,
      102,
      2,
      211,
      78,
      29,
      217,
      141,
      175,
      15,
      146,
      76,
      181,
      137,
      19,
      5,
      44,
      65,
      11,
      202,
      141,
      60,
      24,
      113
    ],
    "record_hash": [
      162,
      115,
      121,
      147,
      121,
      61,
      30,
      195,
      229,
      41,
      132,
      84,
      30,
      24,
      87,
      99,
      101,
      113,
      24,
      229,
      251,
      23,
      201,
      20,
      60,
      243,
      136,
      239,
      226,
      18,
      166,
      135
    ]
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
    "trace_id": "00000000000000000000000000000006",
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
    "record_hash": [
      77,
      238,
      13,
      177,
      144,
      95,
      47,
      132,
      98,
      208,
      212,
      96,
      76,
      222,
      64,
      138,
      112,
      153,
      143,
      116,
      55,
      253,
      193,
      234,
      119,
      203,
      21,
      98,
      190,
      150,
      234,
      212
    ]
  }
]
```
