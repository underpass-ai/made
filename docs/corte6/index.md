# C6 — documentación práctica

Este directorio contiene la entrega C6.13: una guía ejecutable para usar
MADE por MCP local, pedir al agente una ceremonia concreta y escoger una de
las siete arquitecturas de coordinación que el código declara.

## Documentos

- [Guía práctica de MCP local y diseño de ceremonias](c6-13-guia-mcp-local.md)
- [Checklist de aceptación](checklist-aceptacion.md)

## Alcance y evidencia

La guía se redacta contra el checkout actual. Las fuentes principales son:

- `crates/made-mcp/src/protocol/catalog.rs` y
  `crates/made-mcp/src/protocol/tool_names.rs` para el catálogo público.
- `crates/made-mcp/src/protocol/ceremony_schemas/` para entradas y límites.
- `crates/made-mcp/src/protocol/ceremony_pattern_catalog.rs` y
  `crates/made-mcp/src/protocol/fragments/` para las arquitecturas.
- `crates/made-mcp/src/server.rs` y `crates/made-mcp/src/backend.rs` para la
  selección de backend y configuración local.
- `docs/embedded/README.md`, `docs/authoring/README.md` y
  `docs/operations/recoverable-workers.md` para los recorridos operativos.

Cuando algo depende de un proveedor, del proceso anfitrión, de una fuente
configurada o de un permiso, la guía lo marca como tal. Cuando no se puede
comprobar en este árbol, queda como `Pendiente de verificar` y no como una
capacidad.

## Criterio de enlace posterior

El integrador puede enlazar este índice desde la documentación compartida
cuando el checklist esté aceptado. C6.13 no modifica README, `docs/index.md`,
el catálogo compartido ni las skills, y deja fuera el futuro visualizador de
ceremonias.
