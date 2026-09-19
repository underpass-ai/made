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
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:ceremony_instance_started:session",
      "event_type": "ceremony_instance_started",
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
        202,
        30,
        218,
        19,
        60,
        187,
        213,
        211,
        162,
        220,
        107,
        92,
        95,
        168,
        137,
        222,
        97,
        188,
        87,
        103,
        179,
        32,
        148,
        135,
        65,
        146,
        128,
        149,
        95,
        130,
        41,
        117
      ]
    },
    "authorization": {
      "decision_id": "da9937d0e45eb9ca16a285f967b6bf7c9201c9fabc10d244ee893917eb93e93d",
      "request_id": "made-2490fc95e8ca31233dc9f04cf00f42604296ba1469dbcd21141bce363d94cb03",
      "principal_id": "grpc-fixture-host",
      "action": "start_ceremony",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "63f89f0dadc27cc806aec8a656b5c4a5d36035fb2d60e541796b2a2190b9c4da",
      "policy_version": 32,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:participants_bound:seating:FACILITATOR=facilitation",
      "event_type": "participants_bound",
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
        202,
        30,
        218,
        19,
        60,
        187,
        213,
        211,
        162,
        220,
        107,
        92,
        95,
        168,
        137,
        222,
        97,
        188,
        87,
        103,
        179,
        32,
        148,
        135,
        65,
        146,
        128,
        149,
        95,
        130,
        41,
        117
      ],
      "record_hash": [
        189,
        27,
        100,
        95,
        193,
        104,
        97,
        38,
        41,
        217,
        251,
        143,
        40,
        55,
        16,
        169,
        61,
        80,
        173,
        66,
        49,
        19,
        58,
        4,
        28,
        201,
        179,
        98,
        196,
        233,
        103,
        2
      ]
    },
    "authorization": {
      "decision_id": "c13b609c66da5f0831f4ffe36f789c6429b4a95ba1e784c2cc46ce47fe3652b8",
      "request_id": "made-05032c67d54fb0b79dcf694cf7e43ff1c8cd9cc485ad14857b9d736e16bf1afc",
      "principal_id": "grpc-fixture-host",
      "action": "bind_ceremony_participants",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "272139c5cbdffe5c1e65ce89a9d660c82a27768e507bbb06f4c1f8e8fb6a9c4b",
      "policy_version": 33,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:step_started:step:work:visit:1:state_iteration:1:iteration:1:attempt:1",
      "event_type": "step_started",
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
        189,
        27,
        100,
        95,
        193,
        104,
        97,
        38,
        41,
        217,
        251,
        143,
        40,
        55,
        16,
        169,
        61,
        80,
        173,
        66,
        49,
        19,
        58,
        4,
        28,
        201,
        179,
        98,
        196,
        233,
        103,
        2
      ],
      "record_hash": [
        249,
        196,
        168,
        179,
        199,
        45,
        215,
        44,
        157,
        68,
        13,
        214,
        61,
        69,
        85,
        163,
        188,
        104,
        113,
        230,
        102,
        92,
        21,
        197,
        173,
        11,
        237,
        44,
        247,
        142,
        26,
        135
      ]
    },
    "authorization": {
      "decision_id": "f59ab34bb01fc4b1bf761ed253047494f93b1e3c34f72ad71beeae8ecfc5d031",
      "request_id": "made-9e32590f4fca08b14376b40a9d0c6023bae4f0d3a7876b09b8ded9eda04595af",
      "principal_id": "grpc-fixture-host",
      "action": "run_ceremony_step",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "57233777af71752eb010b831e8a8df06fddc831ab5ebd8782930352ae5b1b24d",
      "policy_version": 34,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:step_completed:step:work:visit:1:state_iteration:1:iteration:1:attempt:1",
      "event_type": "step_completed",
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
        249,
        196,
        168,
        179,
        199,
        45,
        215,
        44,
        157,
        68,
        13,
        214,
        61,
        69,
        85,
        163,
        188,
        104,
        113,
        230,
        102,
        92,
        21,
        197,
        173,
        11,
        237,
        44,
        247,
        142,
        26,
        135
      ],
      "record_hash": [
        65,
        43,
        91,
        226,
        187,
        33,
        56,
        188,
        98,
        205,
        117,
        197,
        125,
        109,
        239,
        192,
        227,
        98,
        88,
        111,
        132,
        195,
        243,
        133,
        4,
        81,
        180,
        123,
        18,
        138,
        105,
        24
      ]
    },
    "authorization": {
      "decision_id": "f59ab34bb01fc4b1bf761ed253047494f93b1e3c34f72ad71beeae8ecfc5d031",
      "request_id": "made-9e32590f4fca08b14376b40a9d0c6023bae4f0d3a7876b09b8ded9eda04595af",
      "principal_id": "grpc-fixture-host",
      "action": "run_ceremony_step",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "57233777af71752eb010b831e8a8df06fddc831ab5ebd8782930352ae5b1b24d",
      "policy_version": 34,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:context_written:step:work:visit:1:state_iteration:1:iteration:1:attempt:1",
      "event_type": "context_written",
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
        65,
        43,
        91,
        226,
        187,
        33,
        56,
        188,
        98,
        205,
        117,
        197,
        125,
        109,
        239,
        192,
        227,
        98,
        88,
        111,
        132,
        195,
        243,
        133,
        4,
        81,
        180,
        123,
        18,
        138,
        105,
        24
      ],
      "record_hash": [
        179,
        94,
        120,
        31,
        21,
        226,
        117,
        25,
        139,
        201,
        149,
        196,
        83,
        238,
        89,
        200,
        9,
        48,
        192,
        233,
        61,
        254,
        151,
        243,
        226,
        249,
        87,
        172,
        57,
        171,
        217,
        168
      ]
    },
    "authorization": {
      "decision_id": "f59ab34bb01fc4b1bf761ed253047494f93b1e3c34f72ad71beeae8ecfc5d031",
      "request_id": "made-9e32590f4fca08b14376b40a9d0c6023bae4f0d3a7876b09b8ded9eda04595af",
      "principal_id": "grpc-fixture-host",
      "action": "run_ceremony_step",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "57233777af71752eb010b831e8a8df06fddc831ab5ebd8782930352ae5b1b24d",
      "policy_version": 34,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:transition_applied:transition:1",
      "event_type": "transition_applied",
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
        179,
        94,
        120,
        31,
        21,
        226,
        117,
        25,
        139,
        201,
        149,
        196,
        83,
        238,
        89,
        200,
        9,
        48,
        192,
        233,
        61,
        254,
        151,
        243,
        226,
        249,
        87,
        172,
        57,
        171,
        217,
        168
      ],
      "record_hash": [
        2,
        114,
        88,
        89,
        118,
        33,
        108,
        13,
        39,
        164,
        74,
        64,
        19,
        37,
        49,
        148,
        142,
        225,
        11,
        65,
        148,
        96,
        194,
        33,
        222,
        82,
        98,
        193,
        185,
        34,
        113,
        70
      ]
    },
    "authorization": {
      "decision_id": "674de501a3907da147fc2bbb809ff16b8668eeacda6ae354dfc9ac7b90058f66",
      "request_id": "made-b5bdc342b2b76a3c65efd60e6fccd098decbabb6336cb46769ea42865049ffd1",
      "principal_id": "grpc-fixture-host",
      "action": "apply_ceremony_transition",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "636e6a88a9fefcb277ba20bc9a6de6957c8fb1462c813bbb741a344f15fead42",
      "policy_version": 35,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:step_started:step:handoff:visit:2:state_iteration:1:iteration:1:attempt:1",
      "event_type": "step_started",
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
        2,
        114,
        88,
        89,
        118,
        33,
        108,
        13,
        39,
        164,
        74,
        64,
        19,
        37,
        49,
        148,
        142,
        225,
        11,
        65,
        148,
        96,
        194,
        33,
        222,
        82,
        98,
        193,
        185,
        34,
        113,
        70
      ],
      "record_hash": [
        151,
        74,
        162,
        49,
        174,
        130,
        234,
        23,
        222,
        102,
        118,
        172,
        71,
        167,
        146,
        7,
        54,
        155,
        214,
        194,
        161,
        149,
        252,
        228,
        176,
        153,
        232,
        221,
        49,
        52,
        240,
        184
      ]
    },
    "authorization": {
      "decision_id": "efb1045eef9710acfd9263ac4612572581e0b72a44ac7f9166750b1fd7d245d9",
      "request_id": "made-1300cd058cc200b73924d11dc6a315ed65ed3f68cd7448bee51deb6636d1420b",
      "principal_id": "grpc-fixture-host",
      "action": "claim_ceremony_step",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "cf110e7c292141a45e99f7cde9eb8e06718be8c494871f8f171f4306e845a3a9",
      "policy_version": 36,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:step_completed:step:handoff:visit:2:state_iteration:1:iteration:1:attempt:1",
      "event_type": "step_completed",
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
        151,
        74,
        162,
        49,
        174,
        130,
        234,
        23,
        222,
        102,
        118,
        172,
        71,
        167,
        146,
        7,
        54,
        155,
        214,
        194,
        161,
        149,
        252,
        228,
        176,
        153,
        232,
        221,
        49,
        52,
        240,
        184
      ],
      "record_hash": [
        248,
        161,
        149,
        251,
        104,
        221,
        29,
        230,
        237,
        31,
        108,
        135,
        203,
        97,
        75,
        32,
        166,
        161,
        215,
        108,
        113,
        148,
        109,
        6,
        192,
        242,
        207,
        131,
        148,
        202,
        93,
        46
      ]
    },
    "authorization": {
      "decision_id": "e755c783f746dde9e5a96191c3fd7ddd0580cb578c9dade4f7cf3d7f63c614bc",
      "request_id": "made-7b91285193e9302f7b8b6ca0a4db1e5e8e2eb87f3e380f8a680e00040906f00c",
      "principal_id": "grpc-fixture-host",
      "action": "complete_ceremony_step",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "c3978e80539b5bae1136082413bcbbd9026599267faab5555f8d176440ef513f",
      "policy_version": 37,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:context_written:step:handoff:visit:2:state_iteration:1:iteration:1:attempt:1",
      "event_type": "context_written",
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
        248,
        161,
        149,
        251,
        104,
        221,
        29,
        230,
        237,
        31,
        108,
        135,
        203,
        97,
        75,
        32,
        166,
        161,
        215,
        108,
        113,
        148,
        109,
        6,
        192,
        242,
        207,
        131,
        148,
        202,
        93,
        46
      ],
      "record_hash": [
        63,
        176,
        255,
        74,
        54,
        4,
        168,
        16,
        18,
        64,
        203,
        184,
        39,
        244,
        45,
        62,
        96,
        15,
        8,
        222,
        209,
        107,
        47,
        134,
        149,
        118,
        250,
        209,
        254,
        90,
        39,
        43
      ]
    },
    "authorization": {
      "decision_id": "e755c783f746dde9e5a96191c3fd7ddd0580cb578c9dade4f7cf3d7f63c614bc",
      "request_id": "made-7b91285193e9302f7b8b6ca0a4db1e5e8e2eb87f3e380f8a680e00040906f00c",
      "principal_id": "grpc-fixture-host",
      "action": "complete_ceremony_step",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "c3978e80539b5bae1136082413bcbbd9026599267faab5555f8d176440ef513f",
      "policy_version": 37,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:intervention_requested:intervention:what-happened",
      "event_type": "intervention_requested",
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
        63,
        176,
        255,
        74,
        54,
        4,
        168,
        16,
        18,
        64,
        203,
        184,
        39,
        244,
        45,
        62,
        96,
        15,
        8,
        222,
        209,
        107,
        47,
        134,
        149,
        118,
        250,
        209,
        254,
        90,
        39,
        43
      ],
      "record_hash": [
        168,
        7,
        238,
        10,
        104,
        126,
        90,
        223,
        57,
        131,
        65,
        233,
        125,
        38,
        158,
        212,
        11,
        207,
        4,
        227,
        66,
        62,
        225,
        238,
        231,
        65,
        7,
        252,
        121,
        16,
        168,
        127
      ]
    },
    "authorization": {
      "decision_id": "5b4b30ac93780d1d974dae5dc8a0a7fd643c7b8a78052362be9ab5da9c8ca8d2",
      "request_id": "made-b02ab0c2aaf1f0b8bb90fdaf81d8d2548b9a8960f465d4dfbb95db63e0ef4044",
      "principal_id": "grpc-fixture-host",
      "action": "request_ceremony_intervention",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "0317f432636c9944dc16ddf414dbe5cbf5e07e7342a051b143528250b7dd5757",
      "policy_version": 38,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:intervention_responded:intervention:what-happened:OBSERVER",
      "event_type": "intervention_responded",
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
        168,
        7,
        238,
        10,
        104,
        126,
        90,
        223,
        57,
        131,
        65,
        233,
        125,
        38,
        158,
        212,
        11,
        207,
        4,
        227,
        66,
        62,
        225,
        238,
        231,
        65,
        7,
        252,
        121,
        16,
        168,
        127
      ],
      "record_hash": [
        153,
        27,
        17,
        116,
        71,
        151,
        89,
        137,
        241,
        31,
        61,
        102,
        210,
        205,
        96,
        128,
        117,
        97,
        159,
        171,
        128,
        212,
        125,
        208,
        100,
        151,
        212,
        170,
        255,
        228,
        101,
        128
      ]
    },
    "authorization": {
      "decision_id": "4b9a3e2e2a4bd67148e383c88c6383f0f56fdd048fa4806ed9892138d59cdfe7",
      "request_id": "made-ec5eba901e34fddbb112c5c3e56546cabbd2ba9a99a6a55552cbe6af8cacb377",
      "principal_id": "grpc-fixture-host",
      "action": "respond_to_ceremony_intervention",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "9ac39163fd70531aa3d866fad8ffe50116eef28ee3e600f5d619ebcda289cc2f",
      "policy_version": 39,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:intervention_requested:intervention:inspect-metrics",
      "event_type": "intervention_requested",
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
        27,
        17,
        116,
        71,
        151,
        89,
        137,
        241,
        31,
        61,
        102,
        210,
        205,
        96,
        128,
        117,
        97,
        159,
        171,
        128,
        212,
        125,
        208,
        100,
        151,
        212,
        170,
        255,
        228,
        101,
        128
      ],
      "record_hash": [
        31,
        216,
        58,
        206,
        157,
        126,
        184,
        108,
        163,
        54,
        220,
        50,
        36,
        166,
        180,
        179,
        174,
        146,
        124,
        143,
        73,
        31,
        119,
        61,
        116,
        172,
        85,
        170,
        208,
        121,
        166,
        24
      ]
    },
    "authorization": {
      "decision_id": "40b0fe9fba4d985fb67bc7db73bba348af7bae36b5054e6a6f9ee2584a8e92eb",
      "request_id": "made-894015befeaec9ba5003db5824ee9a96dcb7d9a2b46525432bc73f5fdc9b0604",
      "principal_id": "grpc-fixture-host",
      "action": "request_ceremony_intervention",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "a6a2c8c8efd16092b998ec5066d33f01d47d0318753375c55a60bf7f19aa275a",
      "policy_version": 40,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:evidence_collected:intervention:inspect-metrics:source:observability",
      "event_type": "evidence_collected",
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
        31,
        216,
        58,
        206,
        157,
        126,
        184,
        108,
        163,
        54,
        220,
        50,
        36,
        166,
        180,
        179,
        174,
        146,
        124,
        143,
        73,
        31,
        119,
        61,
        116,
        172,
        85,
        170,
        208,
        121,
        166,
        24
      ],
      "record_hash": [
        83,
        113,
        245,
        122,
        130,
        48,
        1,
        179,
        19,
        129,
        223,
        138,
        168,
        123,
        4,
        37,
        155,
        35,
        232,
        31,
        73,
        245,
        36,
        75,
        184,
        141,
        115,
        164,
        126,
        243,
        155,
        105
      ]
    },
    "authorization": {
      "decision_id": "9e3186d395812e5165103645b4af72250c25083c97de97ea704259bbed9cf92c",
      "request_id": "made-57806e128592576a8a8fdc75bd72e00f811b5992b2c2dcd07bf5a77daeafe936",
      "principal_id": "grpc-fixture-host",
      "action": "collect_ceremony_evidence",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "805831f660ab5e05684b5fd1522a7d9a1207c8265c1d650b8657f89488480b8a",
      "policy_version": 41,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:intervention_responded:intervention:inspect-metrics:OBSERVER",
      "event_type": "intervention_responded",
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
        83,
        113,
        245,
        122,
        130,
        48,
        1,
        179,
        19,
        129,
        223,
        138,
        168,
        123,
        4,
        37,
        155,
        35,
        232,
        31,
        73,
        245,
        36,
        75,
        184,
        141,
        115,
        164,
        126,
        243,
        155,
        105
      ],
      "record_hash": [
        5,
        73,
        89,
        176,
        186,
        0,
        67,
        66,
        141,
        153,
        5,
        156,
        61,
        222,
        107,
        17,
        236,
        128,
        144,
        185,
        13,
        102,
        189,
        6,
        94,
        172,
        153,
        146,
        237,
        74,
        217,
        160
      ]
    },
    "authorization": {
      "decision_id": "9e3186d395812e5165103645b4af72250c25083c97de97ea704259bbed9cf92c",
      "request_id": "made-57806e128592576a8a8fdc75bd72e00f811b5992b2c2dcd07bf5a77daeafe936",
      "principal_id": "grpc-fixture-host",
      "action": "collect_ceremony_evidence",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "805831f660ab5e05684b5fd1522a7d9a1207c8265c1d650b8657f89488480b8a",
      "policy_version": 41,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:reason_asserted:reason:3",
      "event_type": "reason_asserted",
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
        5,
        73,
        89,
        176,
        186,
        0,
        67,
        66,
        141,
        153,
        5,
        156,
        61,
        222,
        107,
        17,
        236,
        128,
        144,
        185,
        13,
        102,
        189,
        6,
        94,
        172,
        153,
        146,
        237,
        74,
        217,
        160
      ],
      "record_hash": [
        65,
        135,
        4,
        138,
        67,
        22,
        54,
        103,
        111,
        45,
        50,
        101,
        154,
        225,
        191,
        66,
        50,
        144,
        134,
        102,
        226,
        126,
        142,
        235,
        12,
        248,
        152,
        156,
        146,
        64,
        140,
        14
      ]
    },
    "authorization": {
      "decision_id": "12c7f03d0a1e4a9a8f51858d560595b025a543dea1dfadb3534c9b5d645b6d88",
      "request_id": "made-0a95021f65d8a5f19bddc86ce6b42b6924f8de295baff2f431742282b4735b52",
      "principal_id": "grpc-fixture-host",
      "action": "assert_ceremony_reason",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "751f3175bca58b491705c76e2a8d8e11ed90e568fc0d8882579c60803f827506",
      "policy_version": 42,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:intervention_closed:intervention:what-happened",
      "event_type": "intervention_closed",
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
        65,
        135,
        4,
        138,
        67,
        22,
        54,
        103,
        111,
        45,
        50,
        101,
        154,
        225,
        191,
        66,
        50,
        144,
        134,
        102,
        226,
        126,
        142,
        235,
        12,
        248,
        152,
        156,
        146,
        64,
        140,
        14
      ],
      "record_hash": [
        13,
        215,
        151,
        223,
        118,
        143,
        56,
        201,
        173,
        69,
        239,
        93,
        128,
        172,
        220,
        88,
        57,
        241,
        58,
        145,
        225,
        6,
        194,
        99,
        160,
        81,
        114,
        255,
        225,
        224,
        99,
        60
      ]
    },
    "authorization": {
      "decision_id": "2ea8171fc0988832f83960bd79867df103638df315810a6b834ae950fb19098f",
      "request_id": "made-114f968574ded9fe9b17ebcdb42049ee71a1ffdf4ced61145d4be70e5eacc595",
      "principal_id": "grpc-fixture-host",
      "action": "close_ceremony_intervention",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "11882bde6d0688b182f08963061706fabd9b4290e454c4577a414766775aed09",
      "policy_version": 43,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:intervention_closed:intervention:inspect-metrics",
      "event_type": "intervention_closed",
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
        13,
        215,
        151,
        223,
        118,
        143,
        56,
        201,
        173,
        69,
        239,
        93,
        128,
        172,
        220,
        88,
        57,
        241,
        58,
        145,
        225,
        6,
        194,
        99,
        160,
        81,
        114,
        255,
        225,
        224,
        99,
        60
      ],
      "record_hash": [
        236,
        174,
        200,
        15,
        80,
        251,
        122,
        50,
        188,
        205,
        36,
        74,
        255,
        42,
        24,
        206,
        109,
        172,
        158,
        91,
        252,
        168,
        46,
        143,
        62,
        139,
        206,
        57,
        100,
        224,
        186,
        145
      ]
    },
    "authorization": {
      "decision_id": "5cc1713aa97d449ab9450350d824908f8a959fa6a067bd10bd1abf82f67187b0",
      "request_id": "made-2ba291875ffb6f5f367d1e6171a4e0987d715e870bd136d454eaa878a5a0b76b",
      "principal_id": "grpc-fixture-host",
      "action": "close_ceremony_intervention",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "efbef7325e3f2256f53e0d6eddf7207e892309fa7603bac15a5a9c41b351c1de",
      "policy_version": 44,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:human_deferral_recorded:guard:human_approved",
      "event_type": "human_deferral_recorded",
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
        236,
        174,
        200,
        15,
        80,
        251,
        122,
        50,
        188,
        205,
        36,
        74,
        255,
        42,
        24,
        206,
        109,
        172,
        158,
        91,
        252,
        168,
        46,
        143,
        62,
        139,
        206,
        57,
        100,
        224,
        186,
        145
      ],
      "record_hash": [
        17,
        186,
        139,
        88,
        253,
        53,
        203,
        140,
        215,
        171,
        241,
        200,
        245,
        117,
        212,
        96,
        228,
        179,
        190,
        75,
        25,
        241,
        212,
        212,
        221,
        162,
        119,
        166,
        199,
        22,
        192,
        79
      ]
    },
    "authorization": {
      "decision_id": "2f88b8cba69afb1233dd1b6225879ab465db04eb905933cf2f0713af1ff2a037",
      "request_id": "made-b8e350ba971a49f92ef484472cfdef33196aebf478e802a3440bbedc01745f7d",
      "principal_id": "grpc-fixture-host",
      "action": "defer_ceremony_guard",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "51f9719da93f344c0cc583a1caef7581e6dc662c023a0323410c3d9d78f01fa7",
      "policy_version": 45,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:human_approval_recorded:guard:human_approved",
      "event_type": "human_approval_recorded",
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
        17,
        186,
        139,
        88,
        253,
        53,
        203,
        140,
        215,
        171,
        241,
        200,
        245,
        117,
        212,
        96,
        228,
        179,
        190,
        75,
        25,
        241,
        212,
        212,
        221,
        162,
        119,
        166,
        199,
        22,
        192,
        79
      ],
      "record_hash": [
        253,
        41,
        141,
        51,
        46,
        2,
        116,
        71,
        35,
        47,
        195,
        22,
        234,
        143,
        72,
        128,
        50,
        160,
        143,
        147,
        167,
        99,
        71,
        162,
        43,
        89,
        97,
        241,
        11,
        174,
        70,
        133
      ]
    },
    "authorization": {
      "decision_id": "b4ea93a431fe59ecc48ed7dd56ea5ee00ce2585c486085e574ea701d1870c82a",
      "request_id": "made-f62cbfdad08ebfe0312d2696ffc93a2e898e41e4c977cba53d338af25404fe8e",
      "principal_id": "grpc-fixture-host",
      "action": "approve_ceremony_guard",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "81a3543635e5c0fde6466c8156617cb9e7f585ef16ddfa8c5f4c724715822406",
      "policy_version": 46,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:transition_applied:transition:2",
      "event_type": "transition_applied",
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
        253,
        41,
        141,
        51,
        46,
        2,
        116,
        71,
        35,
        47,
        195,
        22,
        234,
        143,
        72,
        128,
        50,
        160,
        143,
        147,
        167,
        99,
        71,
        162,
        43,
        89,
        97,
        241,
        11,
        174,
        70,
        133
      ],
      "record_hash": [
        242,
        189,
        10,
        12,
        0,
        78,
        3,
        81,
        137,
        151,
        163,
        251,
        109,
        50,
        16,
        104,
        83,
        34,
        202,
        98,
        120,
        90,
        187,
        78,
        162,
        172,
        137,
        255,
        80,
        179,
        215,
        83
      ]
    },
    "authorization": {
      "decision_id": "4d5c10e3326063c986280245ef7a7bd33600ba0007c65b92261e3738e93bda14",
      "request_id": "made-be4aa6e8f2618c218ce257873ad86079c6d90981a24103846e6bf7b3d3d6f0f9",
      "principal_id": "grpc-fixture-host",
      "action": "apply_ceremony_transition",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "00cc150571c4722ca53508406bebe7bcf8c3e1671d584e2f35999be976bda570",
      "policy_version": 47,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:ceremony_completed:transition:2",
      "event_type": "ceremony_completed",
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
        242,
        189,
        10,
        12,
        0,
        78,
        3,
        81,
        137,
        151,
        163,
        251,
        109,
        50,
        16,
        104,
        83,
        34,
        202,
        98,
        120,
        90,
        187,
        78,
        162,
        172,
        137,
        255,
        80,
        179,
        215,
        83
      ],
      "record_hash": [
        89,
        140,
        229,
        46,
        48,
        130,
        164,
        113,
        165,
        131,
        204,
        220,
        67,
        171,
        36,
        167,
        7,
        231,
        151,
        239,
        131,
        32,
        137,
        56,
        212,
        28,
        109,
        104,
        28,
        6,
        30,
        4
      ]
    },
    "authorization": {
      "decision_id": "4d5c10e3326063c986280245ef7a7bd33600ba0007c65b92261e3738e93bda14",
      "request_id": "made-be4aa6e8f2618c218ce257873ad86079c6d90981a24103846e6bf7b3d3d6f0f9",
      "principal_id": "grpc-fixture-host",
      "action": "apply_ceremony_transition",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-session"
      },
      "target_digest": "00cc150571c4722ca53508406bebe7bcf8c3e1671d584e2f35999be976bda570",
      "policy_version": 47,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
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
    "schema_version": 3,
    "record": {
      "event_id": "parity-published-session:ceremony_instance_started:session",
      "event_type": "ceremony_instance_started",
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
        78,
        141,
        46,
        132,
        86,
        185,
        190,
        82,
        19,
        154,
        219,
        51,
        137,
        169,
        240,
        32,
        20,
        46,
        203,
        126,
        81,
        70,
        69,
        238,
        5,
        86,
        149,
        200,
        191,
        225,
        193,
        205
      ]
    },
    "authorization": {
      "decision_id": "2a765ca10e7712b2b6128d7f1eaa71c944e8dd978a8328307cc9fc7232daeb03",
      "request_id": "made-ee3ae8b6984b628f0672f2697e6cf1d831eda6b379e4870d7ac5de52f765dcf4",
      "principal_id": "grpc-fixture-host",
      "action": "start_published_ceremony",
      "scope": {
        "kind": "ceremony",
        "ceremony_id": "parity-published-session"
      },
      "target_digest": "eefdd42817f88547a32800c9064d120d53aa8ece0d6d8bbeced11b352c83eeca",
      "policy_version": 31,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  }
]
```
