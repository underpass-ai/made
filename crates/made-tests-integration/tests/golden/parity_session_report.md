# Parity review &lt;both arms&gt;

Protocols: 2 · completed: 1 · incomplete: 1

## Protocol `parity-session`

- Definition: `parity_session`
- Version: `1.0`
- Definition digest: `742a2ae8d3fb88175f198182c4efe47b969509e114c1e90321c444b05835e99d`
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
      "event_id": "parity-session:protocol_instance_started:session",
      "event_type": "protocol_instance_started",
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 1,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "parity-operator",
        "kind": "service",
        "role_id": null
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
      "causation_id": null,
      "trace_id": "00000000000000000000000000000007",
      "event_schema_version": 1,
      "event": {
        "type": "protocol_instance_started",
        "protocol_id": "parity-session",
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
        124,
        201,
        33,
        246,
        245,
        164,
        247,
        104,
        162,
        27,
        46,
        98,
        53,
        62,
        176,
        119,
        226,
        219,
        59,
        77,
        71,
        251,
        225,
        18,
        147,
        171,
        105,
        33,
        237,
        253,
        169,
        135
      ]
    },
    "authorization": {
      "decision_id": "25916eb258d1e89b42363bdc3f578d08320bab861c94718d72ff0debb8ecd201",
      "request_id": "made-62df8fda637ff3e789a81661882416433db9aa86cc67fb2e507cf69334b8944f",
      "principal_id": "grpc-fixture-host",
      "action": "start_protocol",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "12a60bbfd67ab1e0f2e2bf72d64194834d3b2d19677ddae4029d237d533d770f",
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
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 2,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "parity-operator",
        "kind": "service",
        "role_id": null
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
      "causation_id": "parity-session:protocol_instance_started:session",
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
        124,
        201,
        33,
        246,
        245,
        164,
        247,
        104,
        162,
        27,
        46,
        98,
        53,
        62,
        176,
        119,
        226,
        219,
        59,
        77,
        71,
        251,
        225,
        18,
        147,
        171,
        105,
        33,
        237,
        253,
        169,
        135
      ],
      "record_hash": [
        9,
        158,
        187,
        251,
        129,
        138,
        29,
        48,
        228,
        100,
        114,
        57,
        44,
        244,
        108,
        142,
        162,
        24,
        60,
        238,
        12,
        121,
        100,
        212,
        155,
        123,
        21,
        175,
        74,
        72,
        248,
        181
      ]
    },
    "authorization": {
      "decision_id": "74a27ff4783f6f199aa7c9f694c361e40d430f254f8ef06b50f58d4f5f433ced",
      "request_id": "made-5d5a88daf6123df0fa01c3533b3af23c806d162ccc44e18634f2f4644a1f8105",
      "principal_id": "grpc-fixture-host",
      "action": "bind_protocol_participants",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "e3602e9332e8acd50c29e02affff2cb9c7437edf29bb0bf70c7eb2be021de326",
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
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 3,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "FACILITATOR",
        "kind": "agent",
        "role_id": "FACILITATOR"
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
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
        9,
        158,
        187,
        251,
        129,
        138,
        29,
        48,
        228,
        100,
        114,
        57,
        44,
        244,
        108,
        142,
        162,
        24,
        60,
        238,
        12,
        121,
        100,
        212,
        155,
        123,
        21,
        175,
        74,
        72,
        248,
        181
      ],
      "record_hash": [
        166,
        200,
        33,
        130,
        9,
        46,
        159,
        44,
        79,
        159,
        247,
        170,
        93,
        149,
        149,
        190,
        44,
        172,
        109,
        205,
        213,
        198,
        140,
        13,
        50,
        254,
        123,
        8,
        80,
        215,
        225,
        214
      ]
    },
    "authorization": {
      "decision_id": "f02c8e7900bb00800ac28c7d87a3e4cb5a4f66ab4632dad8bc13a01334b4d128",
      "request_id": "made-b81ea5c2fbce801a935a5c7e8bb8783323328a1be6a4aeabbf0d3f70844290e1",
      "principal_id": "grpc-fixture-host",
      "action": "run_protocol_step",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "33e985ce8c4f6298cedec9fdcbaae93617c9faccb15bf2b67469667f2b547260",
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
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 4,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "FACILITATOR",
        "kind": "agent",
        "role_id": "FACILITATOR"
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
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
        166,
        200,
        33,
        130,
        9,
        46,
        159,
        44,
        79,
        159,
        247,
        170,
        93,
        149,
        149,
        190,
        44,
        172,
        109,
        205,
        213,
        198,
        140,
        13,
        50,
        254,
        123,
        8,
        80,
        215,
        225,
        214
      ],
      "record_hash": [
        56,
        166,
        111,
        87,
        161,
        131,
        110,
        25,
        51,
        53,
        228,
        104,
        209,
        203,
        70,
        221,
        242,
        175,
        110,
        45,
        80,
        193,
        183,
        251,
        219,
        75,
        114,
        235,
        62,
        1,
        155,
        248
      ]
    },
    "authorization": {
      "decision_id": "f02c8e7900bb00800ac28c7d87a3e4cb5a4f66ab4632dad8bc13a01334b4d128",
      "request_id": "made-b81ea5c2fbce801a935a5c7e8bb8783323328a1be6a4aeabbf0d3f70844290e1",
      "principal_id": "grpc-fixture-host",
      "action": "run_protocol_step",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "33e985ce8c4f6298cedec9fdcbaae93617c9faccb15bf2b67469667f2b547260",
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
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 5,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "FACILITATOR",
        "kind": "agent",
        "role_id": "FACILITATOR"
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
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
        56,
        166,
        111,
        87,
        161,
        131,
        110,
        25,
        51,
        53,
        228,
        104,
        209,
        203,
        70,
        221,
        242,
        175,
        110,
        45,
        80,
        193,
        183,
        251,
        219,
        75,
        114,
        235,
        62,
        1,
        155,
        248
      ],
      "record_hash": [
        209,
        187,
        49,
        135,
        156,
        124,
        221,
        254,
        25,
        188,
        48,
        108,
        8,
        138,
        79,
        253,
        238,
        41,
        188,
        163,
        187,
        100,
        79,
        111,
        122,
        34,
        23,
        88,
        102,
        167,
        169,
        173
      ]
    },
    "authorization": {
      "decision_id": "f02c8e7900bb00800ac28c7d87a3e4cb5a4f66ab4632dad8bc13a01334b4d128",
      "request_id": "made-b81ea5c2fbce801a935a5c7e8bb8783323328a1be6a4aeabbf0d3f70844290e1",
      "principal_id": "grpc-fixture-host",
      "action": "run_protocol_step",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "33e985ce8c4f6298cedec9fdcbaae93617c9faccb15bf2b67469667f2b547260",
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
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 6,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "FACILITATOR",
        "kind": "agent",
        "role_id": "FACILITATOR"
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
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
        209,
        187,
        49,
        135,
        156,
        124,
        221,
        254,
        25,
        188,
        48,
        108,
        8,
        138,
        79,
        253,
        238,
        41,
        188,
        163,
        187,
        100,
        79,
        111,
        122,
        34,
        23,
        88,
        102,
        167,
        169,
        173
      ],
      "record_hash": [
        105,
        177,
        206,
        57,
        219,
        112,
        185,
        17,
        255,
        138,
        57,
        217,
        34,
        242,
        119,
        177,
        24,
        115,
        9,
        230,
        127,
        14,
        45,
        145,
        222,
        212,
        86,
        220,
        63,
        222,
        12,
        171
      ]
    },
    "authorization": {
      "decision_id": "bef9666e8e0c60e068f9896c26f55f8bd4b54a3f8a3b769f02b68432096ed978",
      "request_id": "made-8e1a754502ff2542db9496a65a68f73b898348323ccd328ce6c55dc7d3b999e5",
      "principal_id": "grpc-fixture-host",
      "action": "apply_protocol_transition",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "45d5ae3776f2f669cd09dae451cdb4be9ae69a6448853ea8934e8b56f8a16729",
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
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 7,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "FACILITATOR",
        "kind": "agent",
        "role_id": "FACILITATOR"
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
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
        105,
        177,
        206,
        57,
        219,
        112,
        185,
        17,
        255,
        138,
        57,
        217,
        34,
        242,
        119,
        177,
        24,
        115,
        9,
        230,
        127,
        14,
        45,
        145,
        222,
        212,
        86,
        220,
        63,
        222,
        12,
        171
      ],
      "record_hash": [
        174,
        103,
        125,
        181,
        170,
        17,
        84,
        165,
        127,
        46,
        229,
        133,
        11,
        51,
        137,
        196,
        12,
        84,
        178,
        58,
        241,
        207,
        13,
        251,
        223,
        11,
        158,
        109,
        127,
        31,
        52,
        137
      ]
    },
    "authorization": {
      "decision_id": "b01bf2811affe899a2ad14986b235e5aea745969ef8c7ebee0359c29c85868cd",
      "request_id": "made-817bb9234219379f0e0ad5a6f55640e03b6b0b42af92d369e13b2701dc131927",
      "principal_id": "grpc-fixture-host",
      "action": "claim_protocol_step",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "4d6140d249ea3cc2fa659baef15b1f59be50f5fff742a06b4364c3739b3cd314",
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
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 8,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "FACILITATOR",
        "kind": "agent",
        "role_id": "FACILITATOR"
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
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
        174,
        103,
        125,
        181,
        170,
        17,
        84,
        165,
        127,
        46,
        229,
        133,
        11,
        51,
        137,
        196,
        12,
        84,
        178,
        58,
        241,
        207,
        13,
        251,
        223,
        11,
        158,
        109,
        127,
        31,
        52,
        137
      ],
      "record_hash": [
        0,
        79,
        186,
        238,
        144,
        100,
        244,
        92,
        158,
        168,
        131,
        129,
        196,
        234,
        41,
        243,
        255,
        46,
        81,
        30,
        46,
        8,
        134,
        200,
        126,
        186,
        143,
        76,
        69,
        66,
        136,
        243
      ]
    },
    "authorization": {
      "decision_id": "e5bef4dcc3fa8a7f237082ff4d294e3da13595194c24638f91c736ddcc45c097",
      "request_id": "made-fd71239ee1564a6e4c9f18acc680d4f233ef270d289962213de70c02eadaf4ca",
      "principal_id": "grpc-fixture-host",
      "action": "complete_protocol_step",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "0cabb8e5a0cebc61e8e9106449ef91f9519e63520d7fd8058edca08a081e8652",
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
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 9,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "FACILITATOR",
        "kind": "agent",
        "role_id": "FACILITATOR"
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
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
        0,
        79,
        186,
        238,
        144,
        100,
        244,
        92,
        158,
        168,
        131,
        129,
        196,
        234,
        41,
        243,
        255,
        46,
        81,
        30,
        46,
        8,
        134,
        200,
        126,
        186,
        143,
        76,
        69,
        66,
        136,
        243
      ],
      "record_hash": [
        180,
        148,
        116,
        234,
        142,
        114,
        80,
        73,
        40,
        63,
        38,
        24,
        194,
        158,
        177,
        142,
        10,
        179,
        147,
        218,
        243,
        226,
        47,
        46,
        23,
        12,
        59,
        129,
        148,
        233,
        64,
        190
      ]
    },
    "authorization": {
      "decision_id": "e5bef4dcc3fa8a7f237082ff4d294e3da13595194c24638f91c736ddcc45c097",
      "request_id": "made-fd71239ee1564a6e4c9f18acc680d4f233ef270d289962213de70c02eadaf4ca",
      "principal_id": "grpc-fixture-host",
      "action": "complete_protocol_step",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "0cabb8e5a0cebc61e8e9106449ef91f9519e63520d7fd8058edca08a081e8652",
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
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 10,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "FACILITATOR",
        "kind": "human",
        "role_id": "FACILITATOR"
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
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
        180,
        148,
        116,
        234,
        142,
        114,
        80,
        73,
        40,
        63,
        38,
        24,
        194,
        158,
        177,
        142,
        10,
        179,
        147,
        218,
        243,
        226,
        47,
        46,
        23,
        12,
        59,
        129,
        148,
        233,
        64,
        190
      ],
      "record_hash": [
        52,
        214,
        14,
        227,
        43,
        41,
        40,
        90,
        136,
        220,
        196,
        99,
        5,
        148,
        29,
        215,
        188,
        86,
        224,
        59,
        90,
        70,
        133,
        169,
        229,
        224,
        233,
        86,
        39,
        65,
        162,
        251
      ]
    },
    "authorization": {
      "decision_id": "d6e6edbfe13ea5653c7042536fbb56f64b094552e98203c99f2220dd720eaa69",
      "request_id": "made-6faf46f5b6ef54cdef0df235152172d1e39c133830b39f691e50494ef417de99",
      "principal_id": "grpc-fixture-host",
      "action": "request_protocol_intervention",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "6212b6f277a351ab82caabe6abe1f0eef027b38c2d2b17e019153c7c3484505a",
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
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 11,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "OBSERVER",
        "kind": "agent",
        "role_id": "OBSERVER"
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
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
        52,
        214,
        14,
        227,
        43,
        41,
        40,
        90,
        136,
        220,
        196,
        99,
        5,
        148,
        29,
        215,
        188,
        86,
        224,
        59,
        90,
        70,
        133,
        169,
        229,
        224,
        233,
        86,
        39,
        65,
        162,
        251
      ],
      "record_hash": [
        155,
        86,
        114,
        144,
        174,
        252,
        124,
        117,
        142,
        143,
        122,
        43,
        174,
        9,
        179,
        7,
        138,
        172,
        1,
        168,
        150,
        175,
        44,
        164,
        96,
        88,
        34,
        83,
        86,
        29,
        221,
        68
      ]
    },
    "authorization": {
      "decision_id": "05b589e2c53975472c704bcceb6962110d715b033bf7912904d978bcdd86a4c5",
      "request_id": "made-3ad6383732f581afce572cb1a3e1d78a51adea294e5ffbc2fa22072ba29d667c",
      "principal_id": "grpc-fixture-host",
      "action": "respond_to_protocol_intervention",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "4c27963937031ac679574cb99a56aee89e4628b17e66f50330f36550e881141d",
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
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 12,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "FACILITATOR",
        "kind": "human",
        "role_id": "FACILITATOR"
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
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
        155,
        86,
        114,
        144,
        174,
        252,
        124,
        117,
        142,
        143,
        122,
        43,
        174,
        9,
        179,
        7,
        138,
        172,
        1,
        168,
        150,
        175,
        44,
        164,
        96,
        88,
        34,
        83,
        86,
        29,
        221,
        68
      ],
      "record_hash": [
        206,
        80,
        5,
        68,
        240,
        69,
        10,
        48,
        248,
        220,
        241,
        111,
        95,
        28,
        14,
        172,
        189,
        152,
        230,
        169,
        185,
        133,
        146,
        78,
        38,
        138,
        200,
        115,
        92,
        140,
        169,
        214
      ]
    },
    "authorization": {
      "decision_id": "a659000c01b45902e634ca6cfdb90fc011ec368790f369d6207fa27a234b097a",
      "request_id": "made-572a27dff7350b077f0a7c8034b50f0956da115eddb6cc046cff3a3ae6feab28",
      "principal_id": "grpc-fixture-host",
      "action": "request_protocol_intervention",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "d47200c4026a9c89ae69523f04dbf6bb0585ad3b92ca32e08329e5f40594101d",
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
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 13,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "OBSERVER",
        "kind": "agent",
        "role_id": "OBSERVER"
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
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
        206,
        80,
        5,
        68,
        240,
        69,
        10,
        48,
        248,
        220,
        241,
        111,
        95,
        28,
        14,
        172,
        189,
        152,
        230,
        169,
        185,
        133,
        146,
        78,
        38,
        138,
        200,
        115,
        92,
        140,
        169,
        214
      ],
      "record_hash": [
        15,
        85,
        190,
        66,
        175,
        67,
        218,
        182,
        145,
        22,
        61,
        204,
        90,
        235,
        96,
        245,
        54,
        121,
        208,
        7,
        253,
        239,
        31,
        93,
        136,
        25,
        206,
        178,
        30,
        61,
        158,
        100
      ]
    },
    "authorization": {
      "decision_id": "110678987e0b4ad3ab7f3505b576440ed81dfc13320c0bc9035ea2e893d0f748",
      "request_id": "made-52269972c7c8141405a566c1a47320171e2e27b5db99dfccca6355d74c42ee5c",
      "principal_id": "grpc-fixture-host",
      "action": "collect_protocol_evidence",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "063f252e2ee748e4d4075ef4a7a08294f57e849fa354a8ed993eb74eb5bc6bff",
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
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 14,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "OBSERVER",
        "kind": "agent",
        "role_id": "OBSERVER"
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
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
        15,
        85,
        190,
        66,
        175,
        67,
        218,
        182,
        145,
        22,
        61,
        204,
        90,
        235,
        96,
        245,
        54,
        121,
        208,
        7,
        253,
        239,
        31,
        93,
        136,
        25,
        206,
        178,
        30,
        61,
        158,
        100
      ],
      "record_hash": [
        203,
        37,
        236,
        69,
        161,
        169,
        250,
        40,
        176,
        254,
        69,
        75,
        100,
        106,
        203,
        179,
        101,
        216,
        180,
        179,
        190,
        230,
        206,
        127,
        201,
        189,
        45,
        218,
        73,
        130,
        246,
        230
      ]
    },
    "authorization": {
      "decision_id": "110678987e0b4ad3ab7f3505b576440ed81dfc13320c0bc9035ea2e893d0f748",
      "request_id": "made-52269972c7c8141405a566c1a47320171e2e27b5db99dfccca6355d74c42ee5c",
      "principal_id": "grpc-fixture-host",
      "action": "collect_protocol_evidence",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "063f252e2ee748e4d4075ef4a7a08294f57e849fa354a8ed993eb74eb5bc6bff",
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
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 15,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "OBSERVER",
        "kind": "agent",
        "role_id": "OBSERVER"
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
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
        203,
        37,
        236,
        69,
        161,
        169,
        250,
        40,
        176,
        254,
        69,
        75,
        100,
        106,
        203,
        179,
        101,
        216,
        180,
        179,
        190,
        230,
        206,
        127,
        201,
        189,
        45,
        218,
        73,
        130,
        246,
        230
      ],
      "record_hash": [
        126,
        52,
        17,
        120,
        83,
        77,
        151,
        101,
        31,
        88,
        97,
        221,
        111,
        119,
        159,
        229,
        135,
        31,
        72,
        161,
        203,
        48,
        191,
        186,
        141,
        59,
        88,
        188,
        136,
        84,
        38,
        19
      ]
    },
    "authorization": {
      "decision_id": "37977f25aeb6532b5ac5286e91d1a16c61ca5e3fef7fe1120e814420f9e2a7ba",
      "request_id": "made-a2946c6b701d6ab9742029a8c378047d8ea777cd4df0721802d3755d88c89d96",
      "principal_id": "grpc-fixture-host",
      "action": "assert_protocol_reason",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "80e4a9b22c448fc57824eedbf9f86a39b6441c3c711a1e4a91a27041e0b5c965",
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
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 16,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "FACILITATOR",
        "kind": "human",
        "role_id": "FACILITATOR"
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
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
        126,
        52,
        17,
        120,
        83,
        77,
        151,
        101,
        31,
        88,
        97,
        221,
        111,
        119,
        159,
        229,
        135,
        31,
        72,
        161,
        203,
        48,
        191,
        186,
        141,
        59,
        88,
        188,
        136,
        84,
        38,
        19
      ],
      "record_hash": [
        13,
        167,
        96,
        141,
        236,
        208,
        174,
        245,
        198,
        52,
        12,
        252,
        191,
        37,
        192,
        187,
        116,
        243,
        152,
        180,
        129,
        197,
        52,
        45,
        154,
        118,
        227,
        104,
        130,
        0,
        178,
        57
      ]
    },
    "authorization": {
      "decision_id": "0776b8078cbddf3a166574ef52da15a9fe97bbcd619b429004ed3b85d35dacd3",
      "request_id": "made-55fe6c0652d12f3947b16f566dbafb24a87aa49ef49b48a148a2bf0fb4529a59",
      "principal_id": "grpc-fixture-host",
      "action": "close_protocol_intervention",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "125f4735cb85fde2ad7aa4d7253439ff97409678f1a911608a559c489a75b27e",
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
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 17,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "FACILITATOR",
        "kind": "human",
        "role_id": "FACILITATOR"
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
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
        167,
        96,
        141,
        236,
        208,
        174,
        245,
        198,
        52,
        12,
        252,
        191,
        37,
        192,
        187,
        116,
        243,
        152,
        180,
        129,
        197,
        52,
        45,
        154,
        118,
        227,
        104,
        130,
        0,
        178,
        57
      ],
      "record_hash": [
        158,
        124,
        70,
        57,
        38,
        143,
        95,
        218,
        89,
        171,
        61,
        194,
        248,
        110,
        114,
        88,
        180,
        169,
        102,
        245,
        101,
        206,
        211,
        38,
        159,
        8,
        181,
        174,
        219,
        151,
        238,
        149
      ]
    },
    "authorization": {
      "decision_id": "d1794aba6ccaa2297f5534900e8d626d0c32422713fac11a6ae55b3d4fe09c59",
      "request_id": "made-d8abb5c0829893b630a875ff0b8dfc37f869c5124a7b00c65d3ec02e83d76582",
      "principal_id": "grpc-fixture-host",
      "action": "close_protocol_intervention",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "37d33fb0572bdb495ef914642097cef5871ec528b74357a24e33a875fe963650",
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
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 18,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "FACILITATOR",
        "kind": "human",
        "role_id": "FACILITATOR"
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
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
        158,
        124,
        70,
        57,
        38,
        143,
        95,
        218,
        89,
        171,
        61,
        194,
        248,
        110,
        114,
        88,
        180,
        169,
        102,
        245,
        101,
        206,
        211,
        38,
        159,
        8,
        181,
        174,
        219,
        151,
        238,
        149
      ],
      "record_hash": [
        203,
        20,
        147,
        148,
        180,
        89,
        187,
        100,
        239,
        228,
        174,
        40,
        38,
        77,
        91,
        188,
        93,
        119,
        44,
        60,
        164,
        239,
        69,
        197,
        10,
        107,
        186,
        143,
        57,
        115,
        69,
        38
      ]
    },
    "authorization": {
      "decision_id": "c12a3df5deb2d6ba9fa6c11a81f1acef506fc5614f5c2b70cc28e3f99b28ca62",
      "request_id": "made-b1566f8f28fa46b2afb823b43e0f4ae5c9de06f014e4e4755afb9da5c871dc79",
      "principal_id": "grpc-fixture-host",
      "action": "defer_protocol_guard",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "b3b1eb7c24df0fa178b76887b2113d1304dc7da79fa426d02e1b8196e7b98efb",
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
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 19,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "FACILITATOR",
        "kind": "human",
        "role_id": "FACILITATOR"
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
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
        203,
        20,
        147,
        148,
        180,
        89,
        187,
        100,
        239,
        228,
        174,
        40,
        38,
        77,
        91,
        188,
        93,
        119,
        44,
        60,
        164,
        239,
        69,
        197,
        10,
        107,
        186,
        143,
        57,
        115,
        69,
        38
      ],
      "record_hash": [
        252,
        170,
        47,
        138,
        112,
        29,
        200,
        28,
        167,
        20,
        243,
        191,
        164,
        99,
        177,
        235,
        27,
        131,
        171,
        0,
        16,
        196,
        97,
        150,
        155,
        82,
        88,
        28,
        245,
        20,
        117,
        20
      ]
    },
    "authorization": {
      "decision_id": "232f375c7e08be76c09458f221b01c54ccb75f2ae1b1a31bbba85da52f55ff01",
      "request_id": "made-f309160e6b6e54ac01d6b3e8939fa790b1b4699678e1340916123aa969f6aa6a",
      "principal_id": "grpc-fixture-host",
      "action": "approve_protocol_guard",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "c412943f2ac5c007bce917ea569cac63def97f8b31fe06da863b47e102829d1f",
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
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 20,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "FACILITATOR",
        "kind": "human",
        "role_id": "FACILITATOR"
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
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
        252,
        170,
        47,
        138,
        112,
        29,
        200,
        28,
        167,
        20,
        243,
        191,
        164,
        99,
        177,
        235,
        27,
        131,
        171,
        0,
        16,
        196,
        97,
        150,
        155,
        82,
        88,
        28,
        245,
        20,
        117,
        20
      ],
      "record_hash": [
        87,
        35,
        219,
        130,
        90,
        253,
        216,
        200,
        154,
        147,
        254,
        7,
        124,
        175,
        134,
        218,
        111,
        173,
        80,
        117,
        104,
        44,
        13,
        60,
        223,
        169,
        31,
        143,
        213,
        240,
        74,
        112
      ]
    },
    "authorization": {
      "decision_id": "86b043c5a5f2811f2ae81864c9ab8c49d62f158d2c62c7136aeac6620932e6b2",
      "request_id": "made-60b462794dc093054f9a343484a3d33eb49b4e4766e6f9b2b92f3c3443cd96fb",
      "principal_id": "grpc-fixture-host",
      "action": "apply_protocol_transition",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "c16996dab63b907cc4f5e5d9c1d22c8c7f0c441099509673f4e9e8747c670acd",
      "policy_version": 47,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  },
  {
    "schema_version": 3,
    "record": {
      "event_id": "parity-session:protocol_completed:transition:2",
      "event_type": "protocol_completed",
      "protocol_id": "parity-session",
      "definition_name": "parity_session",
      "definition_version": "1.0",
      "sequence": 21,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "FACILITATOR",
        "kind": "human",
        "role_id": "FACILITATOR"
      },
      "correlation_id": "parity-session:protocol_instance_started:session",
      "causation_id": "parity-session:transition_applied:transition:2",
      "trace_id": "00000000000000000000000000000016",
      "event_schema_version": 1,
      "event": {
        "type": "protocol_completed",
        "final_state": "DONE",
        "completed_at": "2026-09-16T09:00:00Z"
      },
      "previous_record_hash": [
        87,
        35,
        219,
        130,
        90,
        253,
        216,
        200,
        154,
        147,
        254,
        7,
        124,
        175,
        134,
        218,
        111,
        173,
        80,
        117,
        104,
        44,
        13,
        60,
        223,
        169,
        31,
        143,
        213,
        240,
        74,
        112
      ],
      "record_hash": [
        82,
        55,
        88,
        44,
        217,
        115,
        235,
        153,
        192,
        42,
        61,
        58,
        106,
        132,
        13,
        182,
        58,
        149,
        174,
        16,
        205,
        114,
        234,
        145,
        160,
        142,
        51,
        109,
        13,
        135,
        94,
        127
      ]
    },
    "authorization": {
      "decision_id": "86b043c5a5f2811f2ae81864c9ab8c49d62f158d2c62c7136aeac6620932e6b2",
      "request_id": "made-60b462794dc093054f9a343484a3d33eb49b4e4766e6f9b2b92f3c3443cd96fb",
      "principal_id": "grpc-fixture-host",
      "action": "apply_protocol_transition",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-session"
      },
      "target_digest": "c16996dab63b907cc4f5e5d9c1d22c8c7f0c441099509673f4e9e8747c670acd",
      "policy_version": 47,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  }
]
```


## Protocol `parity-published-session`

- Definition: `parity_published`
- Version: `1.0`
- Definition digest: `4bbd1df56d1fcaf39cf3444435df289272f178c7f6c7d262a36eee6fa6bd21ce`
- Bound published digest: `4bbd1df56d1fcaf39cf3444435df289272f178c7f6c7d262a36eee6fa6bd21ce`
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
      "event_id": "parity-published-session:protocol_instance_started:session",
      "event_type": "protocol_instance_started",
      "protocol_id": "parity-published-session",
      "definition_name": "parity_published",
      "definition_version": "1.0",
      "sequence": 1,
      "occurred_at": "2026-09-16T09:00:00Z",
      "actor": {
        "actor_id": "parity-operator",
        "kind": "service",
        "role_id": null
      },
      "correlation_id": "parity-published-session:protocol_instance_started:session",
      "causation_id": null,
      "trace_id": "00000000000000000000000000000006",
      "event_schema_version": 1,
      "event": {
        "type": "protocol_instance_started",
        "protocol_id": "parity-published-session",
        "definition_name": "parity_published",
        "definition_version": "1.0",
        "initial_state": "OPEN",
        "step_ids": [
          "work"
        ],
        "context": {},
        "bound_definition": [
          75,
          189,
          29,
          245,
          109,
          31,
          202,
          243,
          156,
          243,
          68,
          68,
          53,
          223,
          40,
          146,
          114,
          241,
          120,
          199,
          246,
          199,
          210,
          98,
          163,
          110,
          238,
          111,
          166,
          189,
          33,
          206
        ],
        "created_at": "2026-09-16T09:00:00Z"
      },
      "previous_record_hash": null,
      "record_hash": [
        37,
        28,
        105,
        229,
        97,
        87,
        69,
        207,
        226,
        177,
        102,
        166,
        57,
        117,
        206,
        166,
        224,
        98,
        138,
        234,
        238,
        221,
        181,
        190,
        159,
        156,
        57,
        228,
        204,
        147,
        47,
        33
      ]
    },
    "authorization": {
      "decision_id": "4f65720b1e9cfc31fbde8100e57eef85425555eedac229c5579615c4a8d27f9e",
      "request_id": "made-30937e758b2a873e3098ccda45eda179555cc12666a2a565b8871c9303f7ac37",
      "principal_id": "grpc-fixture-host",
      "action": "start_published_protocol",
      "scope": {
        "kind": "protocol",
        "protocol_id": "parity-published-session"
      },
      "target_digest": "df933403b730817a1ac69efda3fa353c0cd952d0616f45c54b66f07b741ba7eb",
      "policy_version": 31,
      "admitted_at": "2026-09-16T09:00:00Z",
      "valid_until": "2026-09-16T09:01:00Z"
    }
  }
]
```
