# ADR 014 — Corte 6: ola 0, contratos y ownership

Estado: aceptado para la primera ola de implementación  
Fecha: 2026-09-19  
Contexto: propuesta de corte 6 y handoff de corte 5

## Decisión

C6 se implementa como incrementos verticales sobre las APIs públicas y los
puertos de C5. La primera ola entrega contratos, un host continuo mínimo, una
política de admisión observable, una frontera de ejecución local explícita y la
guía práctica del MCP. Los frentes restantes se conectan a esos contratos; no
se declara completo un frente por tener sólo documentación o un harness.

La autoridad permanece en el journal y en sus proyecciones públicas. Un daemon,
scheduler, consola o renderer no mantiene una base paralela ni decide permisos.
Los adapters reciben el principal autenticado y el scope ya resuelto por el
host; los campos del payload no pueden sustituirlo.

## Contratos de la ola 0

- **Trabajo:** `operation_id`, `claim_fence`, `lease_owner`, `deadline` y
  `idempotency_key` se conservan en cada handoff. La renovación sólo puede
  aceptar el fence vigente; un dueño antiguo obtiene un rechazo tipado y no
  cambia el journal.
- **Admisión:** una decisión contiene raíz, prioridad, coste/capacidad
  solicitada, razón (`admitted`, `backpressure`, `budget`, `permission` o
  `draining`) y versión de política. El scheduler no inventa saldo ni
  convierte una espera en permiso.
- **Aislamiento:** el modo local confiado se denomina explícitamente. El modo
  Linux aislado limita filesystem, red, procesos, CPU, memoria, duración y
  salida; una credencial sólo se referencia y nunca se copia al recibo.
- **Proveedores:** los perfiles son identificados por nombre, revisión,
  endpoint/método y capacidad declarada. Uso, error, fallback y desconocidos
  se registran como observaciones distintas; la ausencia de coste no es cero.
- **Evidencia:** cada resultado enlaza configuración, versión, caso, digest y
  criterio. Una fixture/no-op sólo acredita wiring; no acredita calidad de
  proveedor ni efecto externo.

## Ownership y orden

Los agentes trabajan con conjuntos de escritura disjuntos. El integrador
reserva `Cargo.toml` raíz, proto/catálogos públicos, wiring común, workflows y
la composición final. Cada entrega debe listar archivos, invariantes, pruebas
ejecutadas y un handoff antes de cruzar otro puerto.

| Ola | Agente | Ownership inicial | Entrega |
|---|---|---|---|
| 1A | A | `crates/made-app/src/workers/` y tests del módulo | host continuo, renovación/fence y scheduler acotado |
| 1B | B | nuevo adapter de ejecución en `crates/made-adapters/src/execution/` y tests propios | límites explícitos y recibo de ejecución |
| 1C | C | `docs/corte6/`, ADRs y guía MCP; no editar código | guía ejecutable, índice y checklist de aceptación |
| 2A | A | conectores sólo tras el handoff del puerto de aislamiento | Git/HTTP de aceptación y conciliación |
| 2B | B | perfiles/proveedores y corpus/evaluación en sus directorios | perfiles locales, rubricado y resultados reproducibles |
| 2C | C | artifacts/retención y consola; compartir contratos sólo por handoff | preview/GC seguro y operación visual/CLI |
| 3 | Integrador | proto, identidad, capacidad, CI y escenarios | integración serial, paridad, evidencia y release candidate |

La UI de C6.7 es una superficie de operación sobre APIs públicas; el
visualizador estructural/temporal de ChronoLoom queda fuera y pertenece a la
propuesta de C7.

## Criterio de handoff

Una entrega está lista cuando compila en la configuración afectada, tiene
pruebas positivas y negativas dirigidas, no rompe la paridad pública, y deja un
recibo reproducible bajo `artifacts/made/corte6/`. Los tests largos sólo se
ejecutan si la entrega declara antes la hipótesis, las métricas y el criterio
de terminación.

## Consecuencias

La primera ola no promete sandbox equivalente fuera de Linux ni exactly-once en
sistemas externos no consultables. Las decisiones de runtime, modelos,
registry, clúster y presupuesto externo se deben fijar antes de aceptar sus
escenarios; mientras tanto, la ruta local reproducible es la de referencia.
