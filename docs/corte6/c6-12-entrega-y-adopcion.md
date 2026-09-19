# C6.12 — Entrega y adopción verificadas

La entrega local se valida desde el árbol candidato que se mergea a `main`.
No se copian `target/`, builds ni credenciales a la evidencia.

## Gate de instalación

```bash
cargo check --workspace
cargo test -p made-console --lib
cargo test -p made-app --lib
cargo test -p made-adapters --features sqlite
cargo run -p made-console -- --help
```

La instalación del plugin/MCP se prueba con los scripts del repositorio y la
guía de `docs/corte6/`. Un upgrade se identifica por commit/tag y comprueba que
lectores anteriores siguen pudiendo consultar sesiones, recibos y cursores.
El dashboard es una lectura ensamblada desde APIs públicas: no abre la base ni
mantiene un estado autoritativo paralelo. Mutaciones siguen pasando por la
autorización existente y apuntan al target exacto.

La aceptación de un despliegue real en Kubernetes, PostgreSQL, registry o un
proveedor comercial requiere esos recursos; esta rama no los inventa ni los
presenta como ejecutados.
