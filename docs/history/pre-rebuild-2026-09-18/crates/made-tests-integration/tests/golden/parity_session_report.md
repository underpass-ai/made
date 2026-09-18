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
      181,
      162,
      234,
      140,
      86,
      84,
      51,
      239,
      214,
      8,
      32,
      120,
      14,
      253,
      211,
      161,
      105,
      69,
      214,
      83,
      89,
      87,
      215,
      242,
      41,
      121,
      16,
      129,
      82,
      10,
      100,
      69
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
      181,
      162,
      234,
      140,
      86,
      84,
      51,
      239,
      214,
      8,
      32,
      120,
      14,
      253,
      211,
      161,
      105,
      69,
      214,
      83,
      89,
      87,
      215,
      242,
      41,
      121,
      16,
      129,
      82,
      10,
      100,
      69
    ],
    "record_hash": [
      200,
      126,
      158,
      43,
      23,
      39,
      210,
      249,
      209,
      138,
      0,
      178,
      12,
      87,
      23,
      0,
      16,
      71,
      237,
      211,
      6,
      36,
      97,
      92,
      215,
      179,
      251,
      69,
      108,
      129,
      139,
      38
    ]
  },
  {
    "event_id": "parity-session:context_written:step:work:state_iteration:1:iteration:1:attempt:1",
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
    "causation_id": "parity-session:step_completed:step:work:state_iteration:1:iteration:1:attempt:1",
    "trace_id": "00000000000000000000000000000009",
    "event_schema_version": 1,
    "event": {
      "type": "context_written",
      "step_id": "work",
      "state_iteration": 1,
      "iteration": 1,
      "attempt": 1,
      "patch": {
        "last_step": "work"
      },
      "written_at": "2026-09-16T09:00:00Z"
    },
    "previous_record_hash": [
      200,
      126,
      158,
      43,
      23,
      39,
      210,
      249,
      209,
      138,
      0,
      178,
      12,
      87,
      23,
      0,
      16,
      71,
      237,
      211,
      6,
      36,
      97,
      92,
      215,
      179,
      251,
      69,
      108,
      129,
      139,
      38
    ],
    "record_hash": [
      181,
      39,
      131,
      78,
      77,
      2,
      170,
      213,
      252,
      19,
      177,
      186,
      25,
      18,
      247,
      232,
      63,
      229,
      138,
      208,
      20,
      109,
      38,
      234,
      12,
      156,
      168,
      1,
      15,
      138,
      149,
      145
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
    "causation_id": "parity-session:context_written:step:work:state_iteration:1:iteration:1:attempt:1",
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
      181,
      39,
      131,
      78,
      77,
      2,
      170,
      213,
      252,
      19,
      177,
      186,
      25,
      18,
      247,
      232,
      63,
      229,
      138,
      208,
      20,
      109,
      38,
      234,
      12,
      156,
      168,
      1,
      15,
      138,
      149,
      145
    ],
    "record_hash": [
      66,
      46,
      35,
      62,
      68,
      96,
      190,
      188,
      95,
      33,
      60,
      223,
      252,
      120,
      134,
      237,
      28,
      70,
      80,
      237,
      58,
      190,
      250,
      60,
      166,
      6,
      187,
      74,
      230,
      248,
      88,
      114
    ]
  },
  {
    "event_id": "parity-session:step_started:step:handoff:state_iteration:1:iteration:1:attempt:1",
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
    "event_schema_version": 3,
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
      "role_from": "next_role",
      "started_at": "2026-09-16T09:00:00Z"
    },
    "previous_record_hash": [
      66,
      46,
      35,
      62,
      68,
      96,
      190,
      188,
      95,
      33,
      60,
      223,
      252,
      120,
      134,
      237,
      28,
      70,
      80,
      237,
      58,
      190,
      250,
      60,
      166,
      6,
      187,
      74,
      230,
      248,
      88,
      114
    ],
    "record_hash": [
      241,
      38,
      25,
      131,
      4,
      66,
      26,
      102,
      230,
      242,
      152,
      19,
      230,
      165,
      119,
      221,
      219,
      63,
      203,
      85,
      198,
      126,
      134,
      184,
      162,
      36,
      121,
      40,
      97,
      123,
      212,
      34
    ]
  },
  {
    "event_id": "parity-session:step_completed:step:handoff:state_iteration:1:iteration:1:attempt:1",
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
      241,
      38,
      25,
      131,
      4,
      66,
      26,
      102,
      230,
      242,
      152,
      19,
      230,
      165,
      119,
      221,
      219,
      63,
      203,
      85,
      198,
      126,
      134,
      184,
      162,
      36,
      121,
      40,
      97,
      123,
      212,
      34
    ],
    "record_hash": [
      215,
      212,
      142,
      119,
      34,
      12,
      233,
      174,
      126,
      153,
      64,
      72,
      144,
      245,
      240,
      228,
      13,
      196,
      14,
      105,
      243,
      140,
      129,
      44,
      130,
      242,
      230,
      59,
      106,
      52,
      200,
      18
    ]
  },
  {
    "event_id": "parity-session:context_written:step:handoff:state_iteration:1:iteration:1:attempt:1",
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
    "causation_id": "parity-session:step_completed:step:handoff:state_iteration:1:iteration:1:attempt:1",
    "trace_id": "0000000000000000000000000000000c",
    "event_schema_version": 1,
    "event": {
      "type": "context_written",
      "step_id": "handoff",
      "state_iteration": 1,
      "iteration": 1,
      "attempt": 1,
      "patch": {
        "final_summary": "the reviewer has it"
      },
      "written_at": "2026-09-16T09:00:00Z"
    },
    "previous_record_hash": [
      215,
      212,
      142,
      119,
      34,
      12,
      233,
      174,
      126,
      153,
      64,
      72,
      144,
      245,
      240,
      228,
      13,
      196,
      14,
      105,
      243,
      140,
      129,
      44,
      130,
      242,
      230,
      59,
      106,
      52,
      200,
      18
    ],
    "record_hash": [
      39,
      73,
      82,
      70,
      252,
      200,
      222,
      66,
      70,
      98,
      250,
      230,
      205,
      53,
      12,
      205,
      223,
      205,
      180,
      64,
      15,
      246,
      232,
      0,
      136,
      59,
      119,
      191,
      186,
      66,
      8,
      56
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
    "causation_id": "parity-session:context_written:step:handoff:state_iteration:1:iteration:1:attempt:1",
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
      39,
      73,
      82,
      70,
      252,
      200,
      222,
      66,
      70,
      98,
      250,
      230,
      205,
      53,
      12,
      205,
      223,
      205,
      180,
      64,
      15,
      246,
      232,
      0,
      136,
      59,
      119,
      191,
      186,
      66,
      8,
      56
    ],
    "record_hash": [
      80,
      76,
      183,
      241,
      16,
      153,
      225,
      69,
      91,
      79,
      124,
      119,
      104,
      61,
      134,
      146,
      21,
      219,
      88,
      12,
      86,
      186,
      79,
      197,
      47,
      49,
      180,
      111,
      142,
      167,
      50,
      0
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
      80,
      76,
      183,
      241,
      16,
      153,
      225,
      69,
      91,
      79,
      124,
      119,
      104,
      61,
      134,
      146,
      21,
      219,
      88,
      12,
      86,
      186,
      79,
      197,
      47,
      49,
      180,
      111,
      142,
      167,
      50,
      0
    ],
    "record_hash": [
      153,
      166,
      55,
      112,
      100,
      15,
      99,
      172,
      144,
      108,
      111,
      97,
      190,
      80,
      235,
      226,
      251,
      141,
      184,
      135,
      66,
      110,
      88,
      27,
      46,
      238,
      118,
      22,
      253,
      82,
      194,
      178
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
      153,
      166,
      55,
      112,
      100,
      15,
      99,
      172,
      144,
      108,
      111,
      97,
      190,
      80,
      235,
      226,
      251,
      141,
      184,
      135,
      66,
      110,
      88,
      27,
      46,
      238,
      118,
      22,
      253,
      82,
      194,
      178
    ],
    "record_hash": [
      114,
      185,
      232,
      212,
      86,
      109,
      164,
      59,
      69,
      205,
      81,
      113,
      190,
      1,
      48,
      168,
      90,
      20,
      124,
      198,
      65,
      228,
      20,
      220,
      81,
      246,
      125,
      44,
      118,
      61,
      249,
      194
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
      114,
      185,
      232,
      212,
      86,
      109,
      164,
      59,
      69,
      205,
      81,
      113,
      190,
      1,
      48,
      168,
      90,
      20,
      124,
      198,
      65,
      228,
      20,
      220,
      81,
      246,
      125,
      44,
      118,
      61,
      249,
      194
    ],
    "record_hash": [
      177,
      45,
      11,
      137,
      98,
      94,
      87,
      111,
      15,
      0,
      163,
      213,
      92,
      106,
      212,
      91,
      160,
      91,
      90,
      242,
      6,
      108,
      123,
      235,
      68,
      8,
      147,
      152,
      201,
      137,
      219,
      88
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
      177,
      45,
      11,
      137,
      98,
      94,
      87,
      111,
      15,
      0,
      163,
      213,
      92,
      106,
      212,
      91,
      160,
      91,
      90,
      242,
      6,
      108,
      123,
      235,
      68,
      8,
      147,
      152,
      201,
      137,
      219,
      88
    ],
    "record_hash": [
      36,
      50,
      204,
      125,
      243,
      52,
      80,
      182,
      151,
      59,
      14,
      240,
      44,
      139,
      174,
      244,
      6,
      190,
      178,
      201,
      213,
      131,
      88,
      235,
      82,
      188,
      107,
      192,
      160,
      183,
      95,
      190
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
      36,
      50,
      204,
      125,
      243,
      52,
      80,
      182,
      151,
      59,
      14,
      240,
      44,
      139,
      174,
      244,
      6,
      190,
      178,
      201,
      213,
      131,
      88,
      235,
      82,
      188,
      107,
      192,
      160,
      183,
      95,
      190
    ],
    "record_hash": [
      140,
      203,
      215,
      15,
      214,
      178,
      88,
      112,
      253,
      17,
      0,
      89,
      205,
      78,
      88,
      6,
      163,
      213,
      94,
      63,
      102,
      16,
      160,
      18,
      179,
      55,
      117,
      104,
      124,
      104,
      17,
      172
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
      140,
      203,
      215,
      15,
      214,
      178,
      88,
      112,
      253,
      17,
      0,
      89,
      205,
      78,
      88,
      6,
      163,
      213,
      94,
      63,
      102,
      16,
      160,
      18,
      179,
      55,
      117,
      104,
      124,
      104,
      17,
      172
    ],
    "record_hash": [
      175,
      206,
      28,
      167,
      124,
      237,
      50,
      58,
      235,
      234,
      135,
      75,
      87,
      127,
      189,
      44,
      222,
      250,
      97,
      6,
      117,
      130,
      58,
      94,
      159,
      188,
      155,
      36,
      142,
      20,
      134,
      56
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
      175,
      206,
      28,
      167,
      124,
      237,
      50,
      58,
      235,
      234,
      135,
      75,
      87,
      127,
      189,
      44,
      222,
      250,
      97,
      6,
      117,
      130,
      58,
      94,
      159,
      188,
      155,
      36,
      142,
      20,
      134,
      56
    ],
    "record_hash": [
      1,
      230,
      35,
      243,
      109,
      170,
      181,
      189,
      248,
      175,
      69,
      116,
      77,
      68,
      165,
      99,
      134,
      12,
      61,
      162,
      83,
      148,
      165,
      77,
      254,
      208,
      19,
      126,
      146,
      95,
      71,
      153
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
      1,
      230,
      35,
      243,
      109,
      170,
      181,
      189,
      248,
      175,
      69,
      116,
      77,
      68,
      165,
      99,
      134,
      12,
      61,
      162,
      83,
      148,
      165,
      77,
      254,
      208,
      19,
      126,
      146,
      95,
      71,
      153
    ],
    "record_hash": [
      247,
      209,
      25,
      119,
      230,
      31,
      129,
      55,
      61,
      8,
      203,
      73,
      103,
      41,
      92,
      179,
      48,
      119,
      102,
      155,
      151,
      77,
      132,
      111,
      180,
      203,
      34,
      109,
      127,
      109,
      150,
      116
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
      247,
      209,
      25,
      119,
      230,
      31,
      129,
      55,
      61,
      8,
      203,
      73,
      103,
      41,
      92,
      179,
      48,
      119,
      102,
      155,
      151,
      77,
      132,
      111,
      180,
      203,
      34,
      109,
      127,
      109,
      150,
      116
    ],
    "record_hash": [
      113,
      160,
      74,
      207,
      100,
      172,
      123,
      233,
      119,
      209,
      232,
      95,
      83,
      214,
      241,
      19,
      48,
      243,
      243,
      128,
      207,
      138,
      1,
      53,
      146,
      3,
      187,
      108,
      200,
      241,
      123,
      169
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
      113,
      160,
      74,
      207,
      100,
      172,
      123,
      233,
      119,
      209,
      232,
      95,
      83,
      214,
      241,
      19,
      48,
      243,
      243,
      128,
      207,
      138,
      1,
      53,
      146,
      3,
      187,
      108,
      200,
      241,
      123,
      169
    ],
    "record_hash": [
      216,
      2,
      81,
      204,
      154,
      102,
      199,
      123,
      116,
      48,
      177,
      180,
      148,
      180,
      86,
      15,
      212,
      125,
      154,
      31,
      0,
      74,
      79,
      235,
      134,
      237,
      229,
      180,
      158,
      61,
      130,
      19
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
      216,
      2,
      81,
      204,
      154,
      102,
      199,
      123,
      116,
      48,
      177,
      180,
      148,
      180,
      86,
      15,
      212,
      125,
      154,
      31,
      0,
      74,
      79,
      235,
      134,
      237,
      229,
      180,
      158,
      61,
      130,
      19
    ],
    "record_hash": [
      228,
      124,
      251,
      172,
      5,
      104,
      191,
      137,
      122,
      38,
      85,
      24,
      200,
      148,
      55,
      3,
      19,
      152,
      177,
      70,
      85,
      16,
      169,
      112,
      199,
      109,
      139,
      82,
      191,
      143,
      230,
      233
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
