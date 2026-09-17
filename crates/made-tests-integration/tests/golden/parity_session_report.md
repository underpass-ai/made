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
    "trace_id": "00000000000000000000000000000009",
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
      77,
      81,
      133,
      115,
      97,
      155,
      151,
      163,
      20,
      210,
      81,
      232,
      109,
      125,
      169,
      209,
      151,
      90,
      219,
      225,
      252,
      161,
      147,
      165,
      94,
      27,
      79,
      135,
      95,
      61,
      136,
      29
    ]
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
    "trace_id": "00000000000000000000000000000009",
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
    "previous_record_hash": [
      77,
      81,
      133,
      115,
      97,
      155,
      151,
      163,
      20,
      210,
      81,
      232,
      109,
      125,
      169,
      209,
      151,
      90,
      219,
      225,
      252,
      161,
      147,
      165,
      94,
      27,
      79,
      135,
      95,
      61,
      136,
      29
    ],
    "record_hash": [
      124,
      237,
      1,
      213,
      249,
      235,
      80,
      203,
      211,
      148,
      122,
      134,
      65,
      143,
      132,
      37,
      100,
      86,
      188,
      23,
      99,
      218,
      36,
      75,
      31,
      225,
      67,
      173,
      145,
      33,
      237,
      43
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
    "causation_id": "parity-session:step_completed:step:work:iteration:1:attempt:1",
    "trace_id": "0000000000000000000000000000000a",
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
    "previous_record_hash": [
      124,
      237,
      1,
      213,
      249,
      235,
      80,
      203,
      211,
      148,
      122,
      134,
      65,
      143,
      132,
      37,
      100,
      86,
      188,
      23,
      99,
      218,
      36,
      75,
      31,
      225,
      67,
      173,
      145,
      33,
      237,
      43
    ],
    "record_hash": [
      73,
      117,
      78,
      89,
      180,
      221,
      109,
      80,
      238,
      7,
      88,
      46,
      204,
      100,
      98,
      187,
      241,
      4,
      22,
      206,
      69,
      103,
      1,
      185,
      232,
      140,
      57,
      89,
      151,
      27,
      95,
      151
    ]
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
    "trace_id": "0000000000000000000000000000000b",
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
    "previous_record_hash": [
      73,
      117,
      78,
      89,
      180,
      221,
      109,
      80,
      238,
      7,
      88,
      46,
      204,
      100,
      98,
      187,
      241,
      4,
      22,
      206,
      69,
      103,
      1,
      185,
      232,
      140,
      57,
      89,
      151,
      27,
      95,
      151
    ],
    "record_hash": [
      99,
      129,
      107,
      240,
      2,
      142,
      177,
      152,
      177,
      143,
      68,
      45,
      248,
      197,
      20,
      64,
      247,
      120,
      6,
      144,
      24,
      175,
      70,
      79,
      218,
      152,
      80,
      24,
      241,
      35,
      175,
      13
    ]
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
    "trace_id": "0000000000000000000000000000000c",
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
    "previous_record_hash": [
      99,
      129,
      107,
      240,
      2,
      142,
      177,
      152,
      177,
      143,
      68,
      45,
      248,
      197,
      20,
      64,
      247,
      120,
      6,
      144,
      24,
      175,
      70,
      79,
      218,
      152,
      80,
      24,
      241,
      35,
      175,
      13
    ],
    "record_hash": [
      99,
      217,
      200,
      113,
      240,
      255,
      186,
      125,
      143,
      138,
      3,
      192,
      53,
      219,
      114,
      153,
      40,
      58,
      45,
      214,
      215,
      111,
      164,
      69,
      41,
      135,
      86,
      250,
      156,
      42,
      133,
      220
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
    "causation_id": "parity-session:step_completed:step:handoff:iteration:1:attempt:1",
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
      99,
      217,
      200,
      113,
      240,
      255,
      186,
      125,
      143,
      138,
      3,
      192,
      53,
      219,
      114,
      153,
      40,
      58,
      45,
      214,
      215,
      111,
      164,
      69,
      41,
      135,
      86,
      250,
      156,
      42,
      133,
      220
    ],
    "record_hash": [
      128,
      200,
      34,
      170,
      180,
      244,
      186,
      50,
      52,
      121,
      113,
      148,
      166,
      175,
      116,
      185,
      155,
      203,
      64,
      75,
      233,
      168,
      67,
      59,
      19,
      14,
      242,
      25,
      217,
      195,
      4,
      117
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
      128,
      200,
      34,
      170,
      180,
      244,
      186,
      50,
      52,
      121,
      113,
      148,
      166,
      175,
      116,
      185,
      155,
      203,
      64,
      75,
      233,
      168,
      67,
      59,
      19,
      14,
      242,
      25,
      217,
      195,
      4,
      117
    ],
    "record_hash": [
      158,
      223,
      3,
      222,
      127,
      127,
      201,
      90,
      53,
      201,
      168,
      131,
      102,
      219,
      98,
      138,
      52,
      233,
      43,
      96,
      5,
      147,
      67,
      89,
      228,
      56,
      88,
      197,
      4,
      207,
      118,
      143
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
      158,
      223,
      3,
      222,
      127,
      127,
      201,
      90,
      53,
      201,
      168,
      131,
      102,
      219,
      98,
      138,
      52,
      233,
      43,
      96,
      5,
      147,
      67,
      89,
      228,
      56,
      88,
      197,
      4,
      207,
      118,
      143
    ],
    "record_hash": [
      17,
      205,
      130,
      141,
      155,
      103,
      158,
      76,
      230,
      79,
      124,
      242,
      59,
      142,
      200,
      241,
      155,
      27,
      1,
      124,
      32,
      144,
      201,
      88,
      220,
      45,
      238,
      84,
      210,
      94,
      22,
      155
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
      17,
      205,
      130,
      141,
      155,
      103,
      158,
      76,
      230,
      79,
      124,
      242,
      59,
      142,
      200,
      241,
      155,
      27,
      1,
      124,
      32,
      144,
      201,
      88,
      220,
      45,
      238,
      84,
      210,
      94,
      22,
      155
    ],
    "record_hash": [
      147,
      144,
      215,
      234,
      54,
      122,
      90,
      123,
      157,
      126,
      40,
      238,
      206,
      20,
      29,
      139,
      164,
      246,
      5,
      139,
      148,
      205,
      83,
      161,
      232,
      144,
      163,
      244,
      128,
      63,
      181,
      238
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
      147,
      144,
      215,
      234,
      54,
      122,
      90,
      123,
      157,
      126,
      40,
      238,
      206,
      20,
      29,
      139,
      164,
      246,
      5,
      139,
      148,
      205,
      83,
      161,
      232,
      144,
      163,
      244,
      128,
      63,
      181,
      238
    ],
    "record_hash": [
      221,
      137,
      133,
      233,
      188,
      189,
      239,
      246,
      52,
      97,
      149,
      154,
      209,
      99,
      238,
      153,
      20,
      96,
      255,
      11,
      215,
      28,
      227,
      227,
      6,
      15,
      131,
      225,
      146,
      65,
      231,
      50
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
      221,
      137,
      133,
      233,
      188,
      189,
      239,
      246,
      52,
      97,
      149,
      154,
      209,
      99,
      238,
      153,
      20,
      96,
      255,
      11,
      215,
      28,
      227,
      227,
      6,
      15,
      131,
      225,
      146,
      65,
      231,
      50
    ],
    "record_hash": [
      243,
      21,
      137,
      161,
      139,
      245,
      43,
      3,
      255,
      147,
      114,
      194,
      25,
      159,
      45,
      134,
      164,
      102,
      68,
      73,
      24,
      83,
      1,
      248,
      60,
      65,
      97,
      175,
      61,
      199,
      192,
      199
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
      243,
      21,
      137,
      161,
      139,
      245,
      43,
      3,
      255,
      147,
      114,
      194,
      25,
      159,
      45,
      134,
      164,
      102,
      68,
      73,
      24,
      83,
      1,
      248,
      60,
      65,
      97,
      175,
      61,
      199,
      192,
      199
    ],
    "record_hash": [
      232,
      176,
      191,
      24,
      16,
      91,
      122,
      25,
      182,
      90,
      69,
      174,
      23,
      21,
      61,
      159,
      238,
      87,
      245,
      79,
      167,
      92,
      25,
      150,
      76,
      138,
      175,
      12,
      50,
      198,
      107,
      171
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
      232,
      176,
      191,
      24,
      16,
      91,
      122,
      25,
      182,
      90,
      69,
      174,
      23,
      21,
      61,
      159,
      238,
      87,
      245,
      79,
      167,
      92,
      25,
      150,
      76,
      138,
      175,
      12,
      50,
      198,
      107,
      171
    ],
    "record_hash": [
      156,
      102,
      100,
      77,
      30,
      200,
      85,
      229,
      79,
      240,
      201,
      237,
      134,
      206,
      121,
      160,
      234,
      31,
      152,
      30,
      77,
      187,
      218,
      29,
      147,
      79,
      205,
      45,
      187,
      16,
      109,
      8
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
      156,
      102,
      100,
      77,
      30,
      200,
      85,
      229,
      79,
      240,
      201,
      237,
      134,
      206,
      121,
      160,
      234,
      31,
      152,
      30,
      77,
      187,
      218,
      29,
      147,
      79,
      205,
      45,
      187,
      16,
      109,
      8
    ],
    "record_hash": [
      207,
      114,
      224,
      250,
      239,
      41,
      30,
      252,
      177,
      171,
      116,
      72,
      244,
      76,
      162,
      81,
      69,
      193,
      243,
      164,
      209,
      34,
      119,
      255,
      114,
      72,
      21,
      32,
      33,
      24,
      21,
      31
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
      207,
      114,
      224,
      250,
      239,
      41,
      30,
      252,
      177,
      171,
      116,
      72,
      244,
      76,
      162,
      81,
      69,
      193,
      243,
      164,
      209,
      34,
      119,
      255,
      114,
      72,
      21,
      32,
      33,
      24,
      21,
      31
    ],
    "record_hash": [
      17,
      54,
      240,
      175,
      37,
      47,
      136,
      128,
      143,
      72,
      194,
      236,
      45,
      88,
      174,
      176,
      197,
      135,
      27,
      150,
      13,
      45,
      229,
      35,
      49,
      160,
      136,
      150,
      166,
      65,
      108,
      94
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
    "previous_record_hash": [
      17,
      54,
      240,
      175,
      37,
      47,
      136,
      128,
      143,
      72,
      194,
      236,
      45,
      88,
      174,
      176,
      197,
      135,
      27,
      150,
      13,
      45,
      229,
      35,
      49,
      160,
      136,
      150,
      166,
      65,
      108,
      94
    ],
    "record_hash": [
      248,
      231,
      118,
      134,
      17,
      93,
      255,
      183,
      77,
      193,
      134,
      249,
      41,
      64,
      127,
      172,
      148,
      39,
      7,
      8,
      78,
      102,
      192,
      28,
      236,
      201,
      106,
      128,
      119,
      22,
      164,
      108
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
      248,
      231,
      118,
      134,
      17,
      93,
      255,
      183,
      77,
      193,
      134,
      249,
      41,
      64,
      127,
      172,
      148,
      39,
      7,
      8,
      78,
      102,
      192,
      28,
      236,
      201,
      106,
      128,
      119,
      22,
      164,
      108
    ],
    "record_hash": [
      42,
      95,
      248,
      216,
      153,
      246,
      51,
      184,
      207,
      3,
      183,
      96,
      230,
      210,
      198,
      113,
      63,
      125,
      194,
      230,
      200,
      102,
      249,
      11,
      194,
      114,
      145,
      72,
      117,
      189,
      97,
      72
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
