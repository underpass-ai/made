# C6.12 — Escenarios de entrega

Los cuatro escenarios comparten el árbol candidato y sólo acreditan las
capacidades que el gate local ejecuta. `scripts/ci/corte6-scenarios.sh` es
seguro y no destructivo; exige un árbol limpio antes de empezar.

| Escenario | Recorrido local | Evidencia adicional necesaria |
|---|---|---|
| Cambio de software | worker, ejecución local confiada, conector durable, receipt y dashboard | repositorio de aceptación y sandbox OCI probada |
| Incidente | scheduler, provider observations, recovery/reconciliation y watch reanudable | broker/réplica fallida y proveedor limitado |
| Investigación | corpus C6, procedencia y guía MCP para diseñar/validar/ejecutar | revisión humana muestreada y dos perfiles locales fijados |
| Operación | retención preview/apply, GC con lease, identidad en transición | PostgreSQL/Kubernetes, backup online y campaña de capacidad |

La ausencia de PostgreSQL, Kubernetes, registry o credenciales de proveedor no
se convierte en un falso positivo: el delivery check local deja esos requisitos
como pendientes explícitos.
