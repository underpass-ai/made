# Parity review &lt;both arms&gt;

Ceremonies: 2 · completed: 1 · incomplete: 1

## Ceremony `parity-session`

- Definition: `parity_session`
- Version: `1.0`
- Definition digest: `8ae417f23d320d1920cf5add922e91e454a494551cf1a7857e3feabc98a38191`
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
      "timeout": null,
      "dynamic_role_binding": {
        "context_key": "next_role",
        "allowed_roles": [
          "FACILITATOR"
        ]
      },
      "context_writes": {
        "final_summary": "handoff_note"
      }
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
      "timeout": null,
      "context_writes": {
        "last_step": "step"
      }
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
    "state_visit": 2,
    "iteration": 1,
    "attempt": 1,
    "lease": null,
    "output": {
      "attachments": 2,
      "handoff_note": "the reviewer has it"
    },
    "error_message": null,
    "claimed_role": "FACILITATOR"
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
    "state_visit": 1,
    "to_state": "REVIEW",
    "applied_by": "FACILITATOR",
    "applied_at": "2026-09-16T09:00:00Z"
  },
  {
    "trigger": "approve",
    "from_state": "REVIEW",
    "state_iteration": 1,
    "state_visit": 2,
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
        "next_role": "FACILITATOR",
        "severity": 2
      },
      "bound_definition": null,
      "created_at": "2026-09-16T09:00:00Z"
    },
    "previous_record_hash": null,
    "record_hash": [
      253,
      177,
      244,
      24,
      245,
      123,
      152,
      53,
      142,
      73,
      93,
      2,
      102,
      196,
      164,
      190,
      52,
      182,
      73,
      26,
      102,
      185,
      95,
      130,
      178,
      138,
      136,
      57,
      73,
      175,
      4,
      30
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
      253,
      177,
      244,
      24,
      245,
      123,
      152,
      53,
      142,
      73,
      93,
      2,
      102,
      196,
      164,
      190,
      52,
      182,
      73,
      26,
      102,
      185,
      95,
      130,
      178,
      138,
      136,
      57,
      73,
      175,
      4,
      30
    ],
    "record_hash": [
      77,
      184,
      207,
      79,
      145,
      171,
      179,
      125,
      248,
      234,
      75,
      120,
      235,
      215,
      139,
      5,
      42,
      228,
      99,
      4,
      197,
      186,
      24,
      162,
      102,
      226,
      138,
      5,
      115,
      110,
      105,
      112
    ]
  },
  {
    "event_id": "parity-session:step_started:step:work:visit:1:state_iteration:1:iteration:1:attempt:1",
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
    "event_schema_version": 4,
    "event": {
      "type": "step_started",
      "step_id": "work",
      "state_visit": 1,
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
      77,
      184,
      207,
      79,
      145,
      171,
      179,
      125,
      248,
      234,
      75,
      120,
      235,
      215,
      139,
      5,
      42,
      228,
      99,
      4,
      197,
      186,
      24,
      162,
      102,
      226,
      138,
      5,
      115,
      110,
      105,
      112
    ],
    "record_hash": [
      194,
      87,
      174,
      7,
      157,
      105,
      238,
      88,
      192,
      72,
      53,
      110,
      31,
      171,
      54,
      130,
      124,
      154,
      91,
      110,
      43,
      6,
      118,
      10,
      126,
      185,
      19,
      232,
      197,
      197,
      38,
      28
    ]
  },
  {
    "event_id": "parity-session:step_completed:step:work:visit:1:state_iteration:1:iteration:1:attempt:1",
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
    "causation_id": "parity-session:step_started:step:work:visit:1:state_iteration:1:iteration:1:attempt:1",
    "trace_id": "00000000000000000000000000000009",
    "event_schema_version": 3,
    "event": {
      "type": "step_completed",
      "step_id": "work",
      "state_visit": 1,
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
      194,
      87,
      174,
      7,
      157,
      105,
      238,
      88,
      192,
      72,
      53,
      110,
      31,
      171,
      54,
      130,
      124,
      154,
      91,
      110,
      43,
      6,
      118,
      10,
      126,
      185,
      19,
      232,
      197,
      197,
      38,
      28
    ],
    "record_hash": [
      75,
      54,
      10,
      216,
      208,
      123,
      225,
      85,
      238,
      232,
      88,
      191,
      29,
      214,
      197,
      222,
      21,
      64,
      75,
      24,
      217,
      183,
      161,
      37,
      104,
      100,
      6,
      19,
      231,
      150,
      148,
      84
    ]
  },
  {
    "event_id": "parity-session:context_written:step:work:visit:1:state_iteration:1:iteration:1:attempt:1",
    "event_type": "context_written",
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
    "causation_id": "parity-session:step_completed:step:work:visit:1:state_iteration:1:iteration:1:attempt:1",
    "trace_id": "00000000000000000000000000000009",
    "event_schema_version": 2,
    "event": {
      "type": "context_written",
      "step_id": "work",
      "state_visit": 1,
      "state_iteration": 1,
      "iteration": 1,
      "attempt": 1,
      "patch": {
        "last_step": "work"
      },
      "written_at": "2026-09-16T09:00:00Z"
    },
    "previous_record_hash": [
      75,
      54,
      10,
      216,
      208,
      123,
      225,
      85,
      238,
      232,
      88,
      191,
      29,
      214,
      197,
      222,
      21,
      64,
      75,
      24,
      217,
      183,
      161,
      37,
      104,
      100,
      6,
      19,
      231,
      150,
      148,
      84
    ],
    "record_hash": [
      78,
      247,
      8,
      19,
      122,
      132,
      220,
      18,
      186,
      88,
      107,
      240,
      138,
      146,
      167,
      58,
      152,
      169,
      213,
      16,
      7,
      67,
      58,
      165,
      250,
      97,
      26,
      86,
      15,
      14,
      105,
      10
    ]
  },
  {
    "event_id": "parity-session:transition_applied:transition:1",
    "event_type": "transition_applied",
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
    "causation_id": "parity-session:context_written:step:work:visit:1:state_iteration:1:iteration:1:attempt:1",
    "trace_id": "0000000000000000000000000000000a",
    "event_schema_version": 3,
    "event": {
      "type": "transition_applied",
      "transition": {
        "trigger": "opened",
        "from_state": "OPEN",
        "state_iteration": 1,
        "state_visit": 1,
        "to_state": "REVIEW",
        "applied_by": "FACILITATOR",
        "applied_at": "2026-09-16T09:00:00Z"
      },
      "destination": {
        "state_visit": 2,
        "step_ids": [
          "handoff"
        ]
      }
    },
    "previous_record_hash": [
      78,
      247,
      8,
      19,
      122,
      132,
      220,
      18,
      186,
      88,
      107,
      240,
      138,
      146,
      167,
      58,
      152,
      169,
      213,
      16,
      7,
      67,
      58,
      165,
      250,
      97,
      26,
      86,
      15,
      14,
      105,
      10
    ],
    "record_hash": [
      248,
      184,
      217,
      242,
      27,
      111,
      183,
      216,
      27,
      212,
      200,
      12,
      3,
      104,
      44,
      213,
      133,
      21,
      23,
      30,
      145,
      241,
      221,
      133,
      122,
      78,
      109,
      224,
      30,
      183,
      232,
      124
    ]
  },
  {
    "event_id": "parity-session:step_started:step:handoff:visit:2:state_iteration:1:iteration:1:attempt:1",
    "event_type": "step_started",
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
    "causation_id": "parity-session:transition_applied:transition:1",
    "trace_id": "0000000000000000000000000000000b",
    "event_schema_version": 4,
    "event": {
      "type": "step_started",
      "step_id": "handoff",
      "state_visit": 2,
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
      "role_from": "next_role",
      "started_at": "2026-09-16T09:00:00Z"
    },
    "previous_record_hash": [
      248,
      184,
      217,
      242,
      27,
      111,
      183,
      216,
      27,
      212,
      200,
      12,
      3,
      104,
      44,
      213,
      133,
      21,
      23,
      30,
      145,
      241,
      221,
      133,
      122,
      78,
      109,
      224,
      30,
      183,
      232,
      124
    ],
    "record_hash": [
      211,
      43,
      170,
      251,
      134,
      182,
      129,
      17,
      43,
      130,
      82,
      126,
      175,
      110,
      8,
      0,
      153,
      166,
      247,
      248,
      155,
      33,
      190,
      193,
      138,
      71,
      105,
      251,
      83,
      136,
      136,
      179
    ]
  },
  {
    "event_id": "parity-session:step_completed:step:handoff:visit:2:state_iteration:1:iteration:1:attempt:1",
    "event_type": "step_completed",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 8,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "FACILITATOR",
      "kind": "agent",
      "role_id": "FACILITATOR"
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:step_started:step:handoff:visit:2:state_iteration:1:iteration:1:attempt:1",
    "trace_id": "0000000000000000000000000000000c",
    "event_schema_version": 3,
    "event": {
      "type": "step_completed",
      "step_id": "handoff",
      "state_visit": 2,
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
      211,
      43,
      170,
      251,
      134,
      182,
      129,
      17,
      43,
      130,
      82,
      126,
      175,
      110,
      8,
      0,
      153,
      166,
      247,
      248,
      155,
      33,
      190,
      193,
      138,
      71,
      105,
      251,
      83,
      136,
      136,
      179
    ],
    "record_hash": [
      231,
      213,
      128,
      197,
      159,
      62,
      191,
      213,
      193,
      12,
      254,
      246,
      242,
      211,
      187,
      197,
      78,
      194,
      113,
      250,
      66,
      63,
      2,
      8,
      11,
      204,
      78,
      55,
      240,
      48,
      207,
      18
    ]
  },
  {
    "event_id": "parity-session:context_written:step:handoff:visit:2:state_iteration:1:iteration:1:attempt:1",
    "event_type": "context_written",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 9,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "FACILITATOR",
      "kind": "agent",
      "role_id": "FACILITATOR"
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:step_completed:step:handoff:visit:2:state_iteration:1:iteration:1:attempt:1",
    "trace_id": "0000000000000000000000000000000c",
    "event_schema_version": 2,
    "event": {
      "type": "context_written",
      "step_id": "handoff",
      "state_visit": 2,
      "state_iteration": 1,
      "iteration": 1,
      "attempt": 1,
      "patch": {
        "final_summary": "the reviewer has it"
      },
      "written_at": "2026-09-16T09:00:00Z"
    },
    "previous_record_hash": [
      231,
      213,
      128,
      197,
      159,
      62,
      191,
      213,
      193,
      12,
      254,
      246,
      242,
      211,
      187,
      197,
      78,
      194,
      113,
      250,
      66,
      63,
      2,
      8,
      11,
      204,
      78,
      55,
      240,
      48,
      207,
      18
    ],
    "record_hash": [
      213,
      233,
      20,
      253,
      76,
      146,
      148,
      251,
      50,
      89,
      234,
      248,
      211,
      13,
      226,
      149,
      248,
      0,
      216,
      45,
      172,
      83,
      52,
      72,
      242,
      98,
      136,
      185,
      53,
      169,
      70,
      164
    ]
  },
  {
    "event_id": "parity-session:intervention_requested:intervention:what-happened",
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
    "causation_id": "parity-session:context_written:step:handoff:visit:2:state_iteration:1:iteration:1:attempt:1",
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
      213,
      233,
      20,
      253,
      76,
      146,
      148,
      251,
      50,
      89,
      234,
      248,
      211,
      13,
      226,
      149,
      248,
      0,
      216,
      45,
      172,
      83,
      52,
      72,
      242,
      98,
      136,
      185,
      53,
      169,
      70,
      164
    ],
    "record_hash": [
      0,
      81,
      79,
      164,
      43,
      44,
      95,
      250,
      207,
      80,
      19,
      120,
      84,
      227,
      72,
      68,
      213,
      232,
      97,
      163,
      216,
      126,
      10,
      197,
      108,
      35,
      124,
      53,
      73,
      148,
      226,
      248
    ]
  },
  {
    "event_id": "parity-session:intervention_responded:intervention:what-happened:OBSERVER",
    "event_type": "intervention_responded",
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
      0,
      81,
      79,
      164,
      43,
      44,
      95,
      250,
      207,
      80,
      19,
      120,
      84,
      227,
      72,
      68,
      213,
      232,
      97,
      163,
      216,
      126,
      10,
      197,
      108,
      35,
      124,
      53,
      73,
      148,
      226,
      248
    ],
    "record_hash": [
      204,
      223,
      210,
      71,
      136,
      133,
      64,
      110,
      240,
      234,
      213,
      97,
      3,
      76,
      157,
      0,
      113,
      249,
      120,
      184,
      195,
      37,
      109,
      202,
      72,
      61,
      114,
      240,
      209,
      209,
      146,
      186
    ]
  },
  {
    "event_id": "parity-session:intervention_requested:intervention:inspect-metrics",
    "event_type": "intervention_requested",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 12,
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
      204,
      223,
      210,
      71,
      136,
      133,
      64,
      110,
      240,
      234,
      213,
      97,
      3,
      76,
      157,
      0,
      113,
      249,
      120,
      184,
      195,
      37,
      109,
      202,
      72,
      61,
      114,
      240,
      209,
      209,
      146,
      186
    ],
    "record_hash": [
      102,
      195,
      116,
      34,
      218,
      38,
      99,
      229,
      48,
      30,
      19,
      103,
      86,
      45,
      222,
      217,
      116,
      18,
      246,
      124,
      230,
      171,
      20,
      60,
      210,
      58,
      134,
      53,
      207,
      9,
      128,
      247
    ]
  },
  {
    "event_id": "parity-session:evidence_collected:intervention:inspect-metrics:source:observability",
    "event_type": "evidence_collected",
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
      102,
      195,
      116,
      34,
      218,
      38,
      99,
      229,
      48,
      30,
      19,
      103,
      86,
      45,
      222,
      217,
      116,
      18,
      246,
      124,
      230,
      171,
      20,
      60,
      210,
      58,
      134,
      53,
      207,
      9,
      128,
      247
    ],
    "record_hash": [
      80,
      237,
      53,
      63,
      138,
      198,
      53,
      49,
      6,
      16,
      19,
      28,
      88,
      217,
      158,
      154,
      137,
      212,
      154,
      187,
      107,
      220,
      10,
      17,
      211,
      64,
      88,
      133,
      138,
      226,
      176,
      78
    ]
  },
  {
    "event_id": "parity-session:intervention_responded:intervention:inspect-metrics:OBSERVER",
    "event_type": "intervention_responded",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 14,
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
      80,
      237,
      53,
      63,
      138,
      198,
      53,
      49,
      6,
      16,
      19,
      28,
      88,
      217,
      158,
      154,
      137,
      212,
      154,
      187,
      107,
      220,
      10,
      17,
      211,
      64,
      88,
      133,
      138,
      226,
      176,
      78
    ],
    "record_hash": [
      195,
      83,
      185,
      78,
      253,
      121,
      250,
      128,
      123,
      108,
      84,
      178,
      199,
      2,
      223,
      140,
      191,
      112,
      134,
      66,
      236,
      54,
      155,
      60,
      131,
      152,
      242,
      144,
      129,
      75,
      94,
      117
    ]
  },
  {
    "event_id": "parity-session:reason_asserted:reason:3",
    "event_type": "reason_asserted",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 15,
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
      195,
      83,
      185,
      78,
      253,
      121,
      250,
      128,
      123,
      108,
      84,
      178,
      199,
      2,
      223,
      140,
      191,
      112,
      134,
      66,
      236,
      54,
      155,
      60,
      131,
      152,
      242,
      144,
      129,
      75,
      94,
      117
    ],
    "record_hash": [
      17,
      214,
      223,
      219,
      99,
      74,
      119,
      237,
      245,
      159,
      57,
      77,
      149,
      9,
      102,
      29,
      67,
      44,
      153,
      180,
      127,
      17,
      148,
      117,
      218,
      48,
      84,
      176,
      58,
      203,
      222,
      211
    ]
  },
  {
    "event_id": "parity-session:intervention_closed:intervention:what-happened",
    "event_type": "intervention_closed",
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
      17,
      214,
      223,
      219,
      99,
      74,
      119,
      237,
      245,
      159,
      57,
      77,
      149,
      9,
      102,
      29,
      67,
      44,
      153,
      180,
      127,
      17,
      148,
      117,
      218,
      48,
      84,
      176,
      58,
      203,
      222,
      211
    ],
    "record_hash": [
      254,
      235,
      93,
      37,
      188,
      30,
      49,
      22,
      5,
      50,
      93,
      173,
      113,
      187,
      206,
      160,
      241,
      7,
      204,
      60,
      193,
      153,
      135,
      54,
      78,
      162,
      43,
      78,
      74,
      196,
      66,
      125
    ]
  },
  {
    "event_id": "parity-session:intervention_closed:intervention:inspect-metrics",
    "event_type": "intervention_closed",
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
      254,
      235,
      93,
      37,
      188,
      30,
      49,
      22,
      5,
      50,
      93,
      173,
      113,
      187,
      206,
      160,
      241,
      7,
      204,
      60,
      193,
      153,
      135,
      54,
      78,
      162,
      43,
      78,
      74,
      196,
      66,
      125
    ],
    "record_hash": [
      15,
      134,
      83,
      190,
      234,
      190,
      221,
      68,
      70,
      37,
      162,
      106,
      172,
      243,
      150,
      223,
      3,
      79,
      245,
      80,
      23,
      118,
      131,
      120,
      13,
      30,
      252,
      39,
      230,
      40,
      31,
      10
    ]
  },
  {
    "event_id": "parity-session:human_deferral_recorded:guard:human_approved",
    "event_type": "human_deferral_recorded",
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
      15,
      134,
      83,
      190,
      234,
      190,
      221,
      68,
      70,
      37,
      162,
      106,
      172,
      243,
      150,
      223,
      3,
      79,
      245,
      80,
      23,
      118,
      131,
      120,
      13,
      30,
      252,
      39,
      230,
      40,
      31,
      10
    ],
    "record_hash": [
      231,
      187,
      8,
      50,
      216,
      221,
      227,
      175,
      93,
      181,
      196,
      159,
      240,
      112,
      241,
      162,
      49,
      120,
      230,
      149,
      17,
      188,
      216,
      54,
      143,
      190,
      203,
      2,
      45,
      137,
      165,
      158
    ]
  },
  {
    "event_id": "parity-session:human_approval_recorded:guard:human_approved",
    "event_type": "human_approval_recorded",
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
      231,
      187,
      8,
      50,
      216,
      221,
      227,
      175,
      93,
      181,
      196,
      159,
      240,
      112,
      241,
      162,
      49,
      120,
      230,
      149,
      17,
      188,
      216,
      54,
      143,
      190,
      203,
      2,
      45,
      137,
      165,
      158
    ],
    "record_hash": [
      20,
      177,
      0,
      80,
      148,
      64,
      131,
      79,
      200,
      6,
      127,
      63,
      180,
      224,
      48,
      5,
      1,
      1,
      155,
      81,
      17,
      102,
      74,
      7,
      88,
      52,
      95,
      91,
      235,
      133,
      246,
      16
    ]
  },
  {
    "event_id": "parity-session:transition_applied:transition:2",
    "event_type": "transition_applied",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 20,
    "occurred_at": "2026-09-16T09:00:00Z",
    "actor": {
      "actor_id": "FACILITATOR",
      "kind": "human",
      "role_id": "FACILITATOR"
    },
    "correlation_id": "parity-session:ceremony_instance_started:session",
    "causation_id": "parity-session:human_approval_recorded:guard:human_approved",
    "trace_id": "00000000000000000000000000000016",
    "event_schema_version": 3,
    "event": {
      "type": "transition_applied",
      "transition": {
        "trigger": "approve",
        "from_state": "REVIEW",
        "state_iteration": 1,
        "state_visit": 2,
        "to_state": "DONE",
        "applied_by": "FACILITATOR",
        "applied_at": "2026-09-16T09:00:00Z"
      },
      "destination": {
        "state_visit": 3,
        "step_ids": []
      }
    },
    "previous_record_hash": [
      20,
      177,
      0,
      80,
      148,
      64,
      131,
      79,
      200,
      6,
      127,
      63,
      180,
      224,
      48,
      5,
      1,
      1,
      155,
      81,
      17,
      102,
      74,
      7,
      88,
      52,
      95,
      91,
      235,
      133,
      246,
      16
    ],
    "record_hash": [
      142,
      232,
      83,
      104,
      205,
      57,
      164,
      119,
      74,
      195,
      223,
      168,
      76,
      240,
      252,
      55,
      240,
      93,
      248,
      236,
      35,
      17,
      45,
      220,
      98,
      217,
      124,
      56,
      3,
      140,
      43,
      239
    ]
  },
  {
    "event_id": "parity-session:ceremony_completed:transition:2",
    "event_type": "ceremony_completed",
    "schema_version": 2,
    "ceremony_id": "parity-session",
    "definition_name": "parity_session",
    "definition_version": "1.0",
    "sequence": 21,
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
      142,
      232,
      83,
      104,
      205,
      57,
      164,
      119,
      74,
      195,
      223,
      168,
      76,
      240,
      252,
      55,
      240,
      93,
      248,
      236,
      35,
      17,
      45,
      220,
      98,
      217,
      124,
      56,
      3,
      140,
      43,
      239
    ],
    "record_hash": [
      118,
      144,
      222,
      84,
      247,
      169,
      184,
      216,
      31,
      187,
      118,
      9,
      98,
      231,
      97,
      37,
      210,
      62,
      232,
      55,
      237,
      201,
      136,
      57,
      213,
      234,
      97,
      82,
      132,
      138,
      177,
      80
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
