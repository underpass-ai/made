# C6.11 — Capacidad, fallos y observabilidad

La campaña se separa en gates dirigidos y tandas de composición. No se afirma
un SLO sin conservar hardware, mezcla, configuración y versión.

## Línea base reproducible

```bash
cargo test -p made-app --lib workers::ceremony_worker_scheduler::tests
cargo test -p made-adapters --features sqlite --lib
cargo check --workspace
```

La evidencia debe conservar sesiones activas, hijos por raíz, tamaño de
journal, blobs, consumidores lentos, espera/rechazo/admisión, recuperación,
lag, RSS y CPU. Las campañas de reinicio o broker caído sólo se añaden cuando
la hipótesis y el criterio de terminación están escritos antes de ejecutarlas.

La instrumentación existente mantiene la separación entre métricas de MADE y
uso declarado por un proveedor. Un token/coste no reportado se conserva como
`unknown`, nunca como cero. Los resultados de una campaña no son autoridad del
journal: sirven para comparar la operación y encontrar límites.
