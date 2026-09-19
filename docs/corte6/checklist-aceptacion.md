# C6.13 — checklist de aceptación

Registrar junto a cada marca la versión del binario, commit, backend,
configuración no secreta y evidencia. No incluir claves, certificados ni
artefactos sensibles.

## Alcance y ownership

- [ ] Sólo se han creado archivos nuevos dentro de `docs/corte6/`.
- [ ] No se ha editado README, `docs/index.md`, catálogo compartido ni una
      skill de otro agente.
- [ ] El contenido no incorpora el futuro visualizador de ceremonias.

## Discovery y configuración

- [ ] El binario se identifica con `made-mcp --version`.
- [ ] `initialize` muestra el backend esperado y la postura TLS esperada.
- [ ] `tools/list` y `made_discover_capabilities` se ejecutan contra la misma
      instalación y no anuncian herramientas ausentes.
- [ ] `made_get_help` responde para `audience: "agent"` y/o `"user"`.
- [ ] El modo embedded usa un path SQLite absoluto, política inicializada,
      trusted host, store id y clave HMAC estable de cursores.
- [ ] El modo gRPC, si se prueba, identifica endpoint, TLS y principal sin
      guardar secretos en la evidencia.
- [ ] Si se usa fixture, la evidencia lo etiqueta como fixture y no como
      proveedor real.

## Ciclo de una ceremonia

- [ ] Una petición de diseño produce una intención con campos que el schema
      acepta, o el agente pregunta por lo que falta.
- [ ] `made_validate_ceremony_draft` identifica correctamente un borrador
      válido y uno inválido.
- [ ] `made_explain_ceremony_draft` explica el mismo borrador sin mutarlo.
- [ ] La publicación fija nombre/versión/digest y rechaza contenido distinto
      bajo una versión ya tomada.
- [ ] Una sesión iniciada desde la versión publicada se puede localizar tras
      reabrir el proceso SQLite.
- [ ] El recorrido ejecuta al menos un paso y consulta la instancia después de
      la mutación.
- [ ] Un claim se completa usando la fence aceptada; una fence inventada o
      reemplazada se rechaza.
- [ ] El informe incluye estado, transcript o eventos, ids y pendientes.

## Fronteras de autoridad

- [ ] Está documentado qué handler/proveedor ejecutó cada paso.
- [ ] Está demostrado que MADE no creó subagentes ni procesos por sorpresa.
- [ ] Una intervención distingue opinión, investigación y acción.
- [ ] La evidencia usa un `source_id` realmente configurado; si no existe, se
      marca como pendiente y no se inventa.
- [ ] La aprobación humana se registra sólo después de una decisión humana.
- [ ] Ningún ejemplo promete permisos, credenciales, calidad de modelo,
      sandbox universal o exactly-once externo sin evidencia específica.

## Catálogo de verbos

- [ ] Cada nombre del catálogo de la guía coincide con
      `crates/made-mcp/src/protocol/tool_names.rs` y el test de herramientas.
- [ ] Los schemas no se han copiado con campos inventados; para una llamada
      real se inspecciona `tools/list` del binario probado.
- [ ] El catálogo cubre discovery, diseño, ejecución, intervención,
      recuperación, historia/informes, councils, budgets, artefactos y
      autorización.
- [ ] Las ausencias de una composición aparecen como filtrado del backend,
      no como fallo de la guía.

## Cuatro peticiones completas

- [ ] La petición de cambio/revisión produce roles, criterios, límite y guard
      humano verificables.
- [ ] La petición de incidente separa lectura de evidencia y acción externa.
- [ ] La petición de investigación conserva referencias, contradicciones y
      permisos de la fuente.
- [ ] La petición de decisión multi-especialista no confunde roles declarados
      con agentes/proveedores realmente registrados.

## Siete arquitecturas

- [ ] `sequential` se valida y ejecuta con orden explícito.
- [ ] `concurrent` se valida con join y techo de capacidad declarado.
- [ ] `broadcast_collect` conserva fan-out y síntesis posterior.
- [ ] `group_chat` tiene manager, límite y fallback.
- [ ] `maker_checker` usa dos roles y límite de revisiones.
- [ ] `handoff` tiene roles, salida humana/fallback y límite de rebotes.
- [ ] `magentic` registra ledger, límite y ruta de atasco.
- [ ] Cada ejemplo indica el host/proveedor o fixture que lo ejecutó.
- [ ] Si se prueban hijos, la evidencia distingue `all`, `any` o `quorum` y
      verifica el journal del hijo.

## Recuperación e informe

- [ ] Se reinicia el proceso embedded y se recupera una sesión publicada sin
      editar SQLite manualmente.
- [ ] Se inspecciona al menos una operación pendiente o se deja constancia de
      que no hubo ninguna.
- [ ] Un marcador de efecto externo sin resultado se presenta como
      conciliación requerida, no como permiso para repetir.
- [ ] Se verifica el journal y se conserva el cursor de continuación cuando
      aplica.
- [ ] El informe final separa observado, inferido, requerido por host,
      requerido por proveedor, requerido por permiso y pendiente.

## Salida

- [ ] La guía permite a una persona localizar el verbo adecuado por intención.
- [ ] Una persona puede escoger una de las siete arquitecturas y explicar su
      límite principal.
- [ ] Una persona puede copiar la plantilla y pedir una ceremonia específica
      sin inventar campos ni permisos.
- [ ] Una instalación identificada recorre discovery → publicación → ejecución
      → informe y conserva evidencia suficiente para repetir el recorrido.
