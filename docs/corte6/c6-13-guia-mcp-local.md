# C6.13 — guía práctica de MADE por MCP local

## 1. Qué resuelve esta guía

El recorrido completo es:

```text
descubrir → diseñar → validar/explicar → publicar → iniciar/ejecutar
→ intervenir → recuperar → informar
```

MADE conserva la definición, el estado de la ceremonia, los eventos y los
resultados que su backend pueda persistir. El host sigue siendo responsable de
los agentes, handlers, proveedores y efectos externos. Una definición no es un
permiso: diseñar o publicar una ceremonia no autoriza por sí mismo una
mutación fuera de MADE.

Esta guía distingue tres niveles:

- **Observado en este checkout:** nombre, schema o comportamiento que se puede
  localizar en el código y sus pruebas.
- **Requiere host/proveedor:** necesita un handler, fuente de evidencia,
  modelo, proceso trabajador o servicio que no crea el MCP por sí solo.
- **Requiere permiso/decisión humana:** necesita autorización explícita,
  principal autenticado o aprobación de una persona.

## 2. Configuración local reproducible

### 2.1 Construir el ejecutable

Desde la raíz del repositorio:

```bash
cargo build -p made-mcp --release --locked
./target/release/made-mcp --version
```

La forma exacta de instalar una release o de registrar el ejecutable en
Codex/otro host depende del paquete y del host. En este checkout sí está
comprobado que `made-mcp` habla MCP por stdin/stdout y que los diagnósticos
van por stderr.

### 2.2 Backend embedded con SQLite

El servidor selecciona el backend mediante `MADE_MCP_BACKEND`. Para el modo
embedded, `MADE_MCP_STORE_PATH`, `MADE_AUTH_POLICY_ID` y
`MADE_AUTH_TRUSTED_HOST_ID` son obligatorios en `try_from_env`; la política
debe estar inicializada antes de servir. La composición también lee la
identidad de store y la clave HMAC de cursores.

Ejemplo POSIX; sustituye los valores y no guardes la clave en el repositorio:

```bash
export MADE_MCP_BACKEND=embedded
export MADE_MCP_STORE_PATH="$PWD/tmp/corte6-ceremonies.sqlite3"
export MADE_AUTH_POLICY_ID=corte6-local-policy
export MADE_AUTH_TRUSTED_HOST_ID=corte6-local-host
export MADE_CEREMONY_STORE_ID=corte6-local-store
export MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY=REEMPLAZAR_POR_64_HEX

mkdir -p "$(dirname "$MADE_MCP_STORE_PATH")"
./target/release/made-mcp bootstrap-authorization "$MADE_MCP_STORE_PATH" \
  --policy-id "$MADE_AUTH_POLICY_ID" \
  --trusted-host-id "$MADE_AUTH_TRUSTED_HOST_ID"

./target/release/made-mcp
```

La clave de cursores debe ser privada, estable entre reinicios y tener 64
caracteres hexadecimales (32 bytes). `bootstrap-authorization` abre una
política ausente y es idempotente para el mismo store, política y propietario;
no crea un permiso universal. Las acciones y scopes necesarios se conceden
aparte mediante la superficie pública de autorización.

`MADE_MCP_EVENT_SINK_PATH` es opcional: si se define, el backend embedded
abre un destino JSON Lines para el feed de eventos. Su uso y retención son una
decisión del host local.

SQLite conserva ceremonias, definiciones publicadas, eventos, cursores,
memoria de sesión y recibos según la composición habilitada. Para que una
sesión sea recuperable se debe iniciar desde una definición publicada; YAML
suministrado o montado puede ser sólo de proceso.

### 2.3 Backend gRPC

El backend `grpc` es el valor por defecto cuando el binario fue compilado con
gRPC, pero exige `MADE_MCP_GRPC_ENDPOINT`; no hace fallback silencioso a
embedded. La configuración observada es:

```bash
export MADE_MCP_BACKEND=grpc
export MADE_MCP_GRPC_ENDPOINT=http://127.0.0.1:50055
./target/release/made-mcp
```

Para TLS, el cliente admite `MADE_MCP_GRPC_TLS_MODE` con `disabled`, `server`
o `mutual`, además de:

```text
MADE_MCP_GRPC_TLS_CA_PATH
MADE_MCP_GRPC_TLS_CERT_PATH
MADE_MCP_GRPC_TLS_KEY_PATH
MADE_MCP_GRPC_TLS_DOMAIN_NAME
```

El modo mutual requiere certificado y clave de cliente. Las rutas, CA,
identidades y grants son responsabilidad del operador/host; no se inventan
credenciales desde una petición MCP.

### 2.4 Fixture

`MADE_MCP_BACKEND=fixture` existe para pruebas de cableado. Una fixture
demuestra que el cliente entiende el protocolo, no que un proveedor real,
modelo o ejecución externa funcione.

### 2.5 Discovery antes de trabajar

El servidor expone `initialize`, `tools/list` y dos herramientas propias:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
```

Después de `notifications/initialized`, consulta:

```json
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"made_discover_capabilities","arguments":{}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"made_get_help","arguments":{"audience":"agent"}}}
```

`initialize` informa backend y postura TLS; `tools/list` es el catálogo
ejecutable filtrado por el backend; `made_discover_capabilities` deriva del
mismo catálogo y agrupa capacidades; `made_get_help` acepta `audience`
`agent` o `user`. No asumas que una herramienta presente en otra instalación
está disponible en ésta.

## 3. Qué coordina MADE y qué hace el host

| Responsabilidad | MADE coordina o persiste | Host/proveedor/permiso necesario |
|---|---|---|
| Definición | Estados, pasos, roles, guards, límites, joins y digest publicado | El texto de intención y la elección de roles/arquitectura |
| Ejecución | Claims, leases, fences, eventos, transiciones, repetición y estados | Handler real, agente/modelo, proceso worker y scheduling si se usan claims |
| Concurrente | Qué hermanos están listos y cuándo un join puede habilitar transición | MADE no crea subagentes ni procesos; el host ejecuta claims aceptados |
| Hijos | Plan determinista, aperturas, lineage, verificación de `CeremonyCompleted` y recuperación por cursor | Definiciones hijas publicadas y un host que ejecute sus pasos |
| Intervención | Agenda, respuestas, cierre, razones y evidencia adjunta | Quién responde, fuente de evidencia configurada y cualquier mutación externa |
| Human gate | Registra aprobación o deferencia; no la infiere | Aprobación humana explícita y principal autorizado |
| Evidencia | Llama a la fuente declarada y adjunta un pack no vacío | `source_id` y adaptador de lectura configurado; la fuente debe ser read-only |
| Proveedores | Registra el handler y su resultado/recibo | Credenciales, endpoint, cuota, modelo, cancelación y calidad del proveedor |
| Recuperación | Recorre eventos/recibos pendientes y evita repetir trabajo cuando existe un resultado adoptable | Conciliación de efectos externos; no hay garantía exactly-once para cualquier sistema |
| Seguridad | Aplica la política y filtra operaciones según el backend | Principal, certificados, grants, scopes y decisión humana cuando corresponda |

La ejecución automática de un `run_ceremony` puede invocar handlers que el
host haya configurado. La ruta de `claim`/`complete` deja el scheduling al
host. En ambos casos MADE coordina el contrato y registra el resultado; no
promete un daemon de workers implícito.

## 4. Ciclo operativo ejecutable

### 4.1 Diseñar

`made_design_ceremony` acepta una intención estructurada. En el código se
observan como campos: `name`, `objective`, `outputs`, `participants`, y,
opcionalmente, `version`, `required_inputs`, `optional_inputs`, `stages`,
`pattern`, `final_approval`, `step_timeout_seconds`, `max_attempts`,
`backoff_seconds`, `max_parallel`, `max_transitions` y `max_bounces`.

Un stage puede ser una etapa simple, un grupo o un patrón. Si la intención no
contiene suficiente información, el agente debe preguntar; no debe rellenar
roles, fuentes de evidencia o permisos con nombres inventados.

### 4.2 Validar y explicar

`made_validate_ceremony_draft` y `made_explain_ceremony_draft` reciben sólo:

```json
{"definition_yaml":"<contenido YAML completo>"}
```

El primero analiza defectos y dice si el borrador es publicable; el segundo
explica el mismo análisis para lectura humana. Son operaciones de lectura: no
publican ni ejecutan.

### 4.3 Publicar

`made_publish_ceremony_definition` también recibe `definition_yaml`. Sólo
publica un borrador válido, fija identidad nombre/versión/digest y hace
inmutable esa versión. El mismo contenido repetido es un no-op; contenido
distinto bajo una versión tomada se rechaza. Cambiar el contenido implica
cambiar la versión.

### 4.4 Iniciar y ejecutar

Para una sesión recuperable, `made_start_published_ceremony` nombra la
definición con `ceremony` y `version`, y recibe `actor_id`, `actor_kind`; el
schema también permite `ceremony_id` y `context`. El actor declarado es
proveniencia del llamador, no sustituye al principal autenticado.

`made_run_ceremony` es la ruta de una sola llamada sobre YAML y recibe
`definition_yaml`, `actor_id`, `actor_kind`; el código embedded permite además
`ceremony_id`, `context`, `lease_owner_id` y `lease_ttl_ms`. Para control fino
se usan `made_claim_ceremony_step` y `made_complete_ceremony_step`, pasando la
fence que devuelve el claim; no se debe completar con una fence reconstruida.

Después de cada mutación, consulta `made_get_ceremony_instance` para ver
pasos claimables, guards bloqueantes, transiciones habilitadas y estado actual.

### 4.5 Intervenir

`made_request_ceremony_intervention` abre una petición de tipo `opinion`,
`investigation` o `action`. Los campos requeridos comprobados son
`ceremony_id`, `role_id`, `role_kind`, `kind` y `message`; puede incluir
`intervention_id`, `target_role_ids`, `details` y `provenance`.

`made_respond_to_ceremony_intervention` registra una respuesta dirigida y
`made_close_ceremony_intervention` la cierra desde el rol solicitante.
`made_approve_ceremony_guard` sólo registra una aprobación humana ya concedida;
`made_defer_ceremony_guard` registra deferencia y deja el guard insatisfecho.

### 4.6 Recuperar

Para hijos se usan `made_prepare_ceremony_children`,
`made_accept_child_completion` y `made_recover_ceremony_children`. Para
trabajo externo, `made_get_execution_receipt`,
`made_inspect_execution_recovery`, `made_complete_execution_receipt` y
`made_adopt_execution_receipt` permiten inspeccionar, completar o adoptar un
resultado válido. `made_pull_ceremony_events`, journals y cursores son la
fuente durable; una notificación de broker sólo despierta el trabajo.

La recuperación no repite ciegamente un efecto externo cuyo marcador durable
existe sin resultado. Esa situación se informa para conciliación del operador.

### 4.7 Informar

Para seguir o auditar una sesión: `made_read_ceremony_events`,
`made_stream_ceremony`, `made_get_ceremony_transcript`,
`made_verify_ceremony_journal` y `made_get_ceremony_instance`.

Para un informe: `made_generate_ceremony_report`. Para encontrar sesiones
perdidas por el contexto conversacional: `made_list_ceremony_instances` y
`made_search_ceremony_instances`. Para resultados grandes: artefactos con
`made_begin_artifact_upload`, `made_put_artifact_chunk`,
`made_commit_artifact_upload`, `made_get_artifact` y sus operaciones de lectura.

## 5. Catálogo público por intención

La lista siguiente se extrajo de `GRPC_TOOL_NAMES` y del catálogo de servidor.
En una instalación concreta, `tools/list` es la autoridad final: el backend
puede filtrar herramientas. Los schemas completos siguen siendo la autoridad
para campos y tipos; esta guía no duplica esos schemas.

### Descubrir y pedir ayuda

- `made_discover_capabilities` — backend, versión, grupos, herramientas y generadores activos.
- `made_get_help` — ayuda para `user` o `agent`.
- `made_get_status`, `made_get_metrics` — estado y métricas del servicio/backend.

### Diseñar, analizar y versionar ceremonias

- `made_design_ceremony` — convierte una intención estructurada en un borrador.
- `made_validate_ceremony_draft` — análisis estructural de un YAML.
- `made_explain_ceremony_draft` — explicación legible del análisis.
- `made_publish_ceremony_definition` — publicación inmutable de un YAML válido.
- `made_diff_ceremony_definitions` — diferencia dos referencias de definición.

### Iniciar, ejecutar y consultar sesiones

- `made_run_ceremony` — ejecuta una definición YAML y devuelve estado/trace.
- `made_start_ceremony` — monta YAML y crea una instancia sin avanzarla.
- `made_start_published_ceremony` — inicia una versión publicada y la liga a su digest.
- `made_run_ceremony_step` — ejecuta un paso de una instancia.
- `made_get_ceremony_instance` — estado persistente, pasos y guards.
- `made_list_ceremony_instances` — lista sesiones recuperables del backend.
- `made_search_ceremony_instances` — página acotada por cursor, prefijo o lifecycle.
- `made_apply_ceremony_transition` — aplica una transición habilitada.
- `made_bind_ceremony_participants` — asienta roles y especialidades.

### Ciclo de vida y autorización humana

- `made_pause_ceremony` — detiene admisión nueva y deja drenar claims aceptados.
- `made_resume_ceremony` — reanuda admisión sin desplazar deadlines absolutos.
- `made_cancel_ceremony` — finaliza irreversiblemente la sesión; no revierte efectos externos.
- `made_enforce_ceremony_deadlines` — evalúa y persiste decisiones de timeout.
- `made_approve_ceremony_guard` — registra aprobación humana explícita.
- `made_defer_ceremony_guard` — registra deferencia sin satisfacer el guard.

### Participación, evidencia y razones

- `made_request_ceremony_intervention` — abre opinión, investigación o acción.
- `made_respond_to_ceremony_intervention` — añade respuesta de un rol dirigido.
- `made_close_ceremony_intervention` — cierra una intervención abierta.
- `made_collect_ceremony_evidence` — pide un pack no vacío a una fuente read-only configurada.
- `made_assert_ceremony_reason` — registra por qué una producción llevó a otra.

### Claims, recibos y recuperación de ejecución

- `made_claim_ceremony_step` — reclama un paso con lease/fence.
- `made_complete_ceremony_step` — completa el claim aceptado usando su fence.
- `made_get_execution_receipt` — lee un recibo de ejecución.
- `made_inspect_execution_recovery` — página de operaciones que requieren recuperación.
- `made_complete_execution_receipt` — registra el resultado de una ejecución externa.
- `made_adopt_execution_receipt` — adopta un resultado durable válido tras reemplazo.

### Hijos y recuperación durable

- `made_prepare_ceremony_children` — sella y abre el plan de hijos una vez.
- `made_accept_child_completion` — verifica y acepta la terminación de un hijo.
- `made_recover_ceremony_children` — avanza recuperación desde el cursor de eventos.

### Historia, seguimiento e informes

- `made_read_ceremony_events` — lee eventos sellados.
- `made_stream_ceremony` — sigue una sesión y devuelve cursor de continuación.
- `made_pull_ceremony_events` — consume el feed global por cursor/lease.
- `made_get_ceremony_transcript` — proyecta el transcript.
- `made_generate_ceremony_report` — genera un informe persistible/proyectado.
- `made_verify_ceremony_journal` — verifica integridad y orden del journal.

### Councils, agentes, contratos y journal de councils

- `made_deliberate`, `made_stream_deliberation`, `made_get_deliberation_result` — deliberación y consulta de resultado.
- `made_orchestrate`, `made_process_trigger_event`, `made_run_council_decision` — ejecución/orquestación de councils; el executor/provider debe existir para trabajo externo.
- `made_create_council`, `made_list_councils`, `made_delete_council` — configuración de councils.
- `made_register_agent`, `made_unregister_agent` — registro de agentes.
- `made_register_contract`, `made_list_contracts`, `made_delete_contract` — contratos de salida.
- `made_read_council_events`, `made_get_council_event_cursor`, `made_lease_council_events`, `made_acknowledge_council_events`, `made_release_council_events` — journal/cursor independiente de councils.

### Presupuestos

- `made_get_budget_report` — balance y uso de presupuesto de una ceremonia.
- `made_list_pending_budget_reservations` — reservas pendientes que requieren atención.

### Artefactos

- `made_begin_artifact_upload`, `made_put_artifact_chunk`, `made_commit_artifact_upload`, `made_abort_artifact_upload` — subida resumible por chunks.
- `made_get_artifact`, `made_list_artifacts`, `made_read_artifact_chunk` — lectura/listado.
- `made_tombstone_artifact` — marca un artefacto como tombstone; requiere la autorización correspondiente.

### Administración de autorización

- `made_get_authorization_policy` — lectura de la política.
- `made_approve_authorization_operation` — aprobación de una operación pendiente.
- `made_issue_authorization_grant` — concede un grant explícito.
- `made_revoke_authorization_grant` — revoca un grant.
- `made_list_authorization_decisions` — historial/página de decisiones.

## 6. Plantilla de petición al agente

Copiar y completar esta petición. El agente debe empezar por discovery y
señalar cualquier dato que no pueda verificar:

```text
Quiero una ceremonia MADE para [objetivo único: decisión o artefacto].

Contexto e inputs:
- Contexto disponible: [claves y contenido resumido].
- Inputs obligatorios: [lista].
- Fuentes de evidencia autorizadas: [source_id exacto o "ninguna"].

Roles y autoridad:
- Roles: [role_id → especialidad/responsabilidad].
- Actor que solicita: [actor_id, actor_kind].
- Aprobación humana necesaria: [sí/no; quién y en qué guard].
- Efectos externos permitidos: [ninguno o lista exacta].

Coordinación:
- Arquitectura preferida: [sequential/concurrent/broadcast_collect/group_chat/
  maker_checker/handoff/magentic] o "elige y justifica".
- Número máximo de agentes/iteraciones/hijos: [límites].
- Presupuesto: [límites conocidos o "descubrir primero"].
- Fallback/intervención: [condición y rol humano].

Calidad y salida:
- Criterios verificables: [checks].
- Evidencia que debe quedar: [eventos, transcript, report, artefacto].
- Resultado esperado: [campos/contenido].

Procedimiento obligatorio:
1. Ejecuta discovery y confirma las herramientas y backend realmente activos.
2. Propón la definición y explica qué parte es MADE y qué parte es host/proveedor.
3. Valida; no publiques ni ejecutes hasta que yo lo autorice si hay efectos.
4. Publica sólo si la versión y digest son los esperados.
5. Ejecuta, intervén o recupera sólo dentro de los permisos indicados.
6. Entrega el informe con ids, estado, evidencia, límites y pendientes.
No inventes nombres de roles, source_id, campos, permisos ni resultados.
```

## 7. Cuatro peticiones completas

Son ejemplos de lenguaje para pedir al agente. Cada una exige discovery y
puede tener que adaptarse al catálogo realmente anunciado.

### 7.1 Cambio de software con revisión

```text
Diseña y ejecuta una ceremonia para revisar el cambio "cambio-api-42".
Objetivo: decidir si el cambio puede integrarse sin romper el contrato público.

Contexto: el diff y los tests están en el checkout que el host te indique;
no modifiques archivos ni publiques nada. Inputs: resumen del cambio, lista de
tests y digest del checkout. Roles: IMPLEMENTER produce el plan de pruebas;
REVIEWER revisa contrato y regresiones; HUMAN_APPROVER decide el cierre.

Elige maker_checker si el catálogo lo anuncia; limita a dos roles especialistas,
dos intentos por paso y una revisión de salida. Usa una aprobación humana
explícita para integrar. No ejecutes git push, releases ni cambios de producción.

Criterios: tests relevantes pasan, la definición pública no se contradice, el
reviewer registra objeciones resueltas y el digest observado queda en el
informe. Si no existe un handler de checkout o el permiso de lectura no está
disponible, deja la sesión bloqueada con esa razón. Entrega definición,
validación, digest publicado si fue autorizado, estado final, transcript,
informe y pendientes.
```

### 7.2 Incidente

```text
Investiga el incidente INC-2026-0919 sin ejecutar mutaciones externas.
Objetivo: producir una línea temporal, hipótesis priorizadas y una acción
recomendada, no aplicar el cambio.

Contexto: ventana UTC [inicio, fin], servicios [lista], síntomas [texto].
Roles: INCIDENT_LEAD coordina; OBSERVABILITY lee la fuente de observabilidad;
STORAGE analiza persistencia; HUMAN_ONCALL decide cualquier acción.

Prefiero broadcast_collect: fan-out independiente y un collector. Usa
made_collect_ceremony_evidence sólo con un source_id que discovery y el host
confirmen. Si la fuente no existe, registra "evidencia no disponible" y no la
inventes. Presupuesto: [límite conocido]; si el host no lo expone, informa que
queda pendiente. Intervención humana: abrir una action sólo para proponer, y
aprobarla por separado si la persona decide ejecutarla.

Salida verificable: timeline con referencias, evidencias por hipótesis,
contradicciones, severidad/certeza declaradas, recomendación y un informe
persistido. No llames a un executor ni a una API de producción. Recupera la
sesión si el proceso se reinicia y explica cualquier operación pendiente.
```

### 7.3 Investigación con evidencia

```text
Construye una investigación reproducible sobre "¿por qué aumentó el tiempo de
respuesta?" usando sólo lecturas autorizadas.

Inputs: periodo, servicio, consultas permitidas y referencias de artefactos.
Roles: PERF especialista de métricas, DB especialista de almacenamiento,
DOMAIN especialista del flujo, EDITOR integra la respuesta. Arquitectura:
concurrent para las tres lecturas y luego síntesis; elige
broadcast_collect si está disponible. No uses proveedores remotos ni escribas
en las fuentes.

Primero confirma qué source_id, artefactos y herramientas están disponibles.
Cada respuesta debe incluir consulta, ventana, referencia, resultado y límites.
Si dos fuentes contradicen otra, conserva ambas y abre una intervención de
investigation al rol que pueda resolverla; no elijas por intuición. Límite:
tres especialistas, una síntesis y un reintento técnico por paso.

Entrega: definición validada, evidencia adjunta no vacía cuando la fuente lo
permita, tabla de hipótesis con confianza, conclusión diferenciando hecho de
inferencia, transcript, journal verificado y reporte. Si falta autorización,
detén la lectura y reporta el permiso exacto que falta.
```

### 7.4 Decisión multi-especialista

```text
Necesito decidir si adoptamos la estrategia "S" para el próximo ciclo.
Objetivo: una recomendación con razones auditables; ninguna compra, despliegue
ni cambio externo.

Roles: SECURITY, OPERATIONS, PRODUCT y FINANCE, cada uno con su propio análisis;
DECISION_LEAD sintetiza. Inputs: alternativas, restricciones, presupuesto y
criterios de éxito. Usa broadcast_collect con síntesis; si sólo hay un council
configurado, descubre su especialidad y deja claro qué participantes reales
faltan. No declares que hay cuatro modelos/agentes si el host no los registra.

Límites: cuatro contribuciones como máximo, un paso de síntesis, sin ciclos,
con presupuesto compartido que el backend anuncie. Criterios: cada alternativa
se evalúa contra todos los criterios, se preservan desacuerdos y la decisión
incluye trade-offs y condiciones de reversión. La aprobación final la hace
HUMAN_APPROVER mediante un guard humano; diseñar la ceremonia no cuenta como
aprobación.

Publica sólo después de que yo confirme nombre, versión y digest. Ejecuta y
genera el informe; no ejecutes efectos externos. Devuelve votos/entradas,
síntesis, razones registradas, estado del guard, transcript y artefactos o
pendientes.
```

## 8. Las siete arquitecturas declaradas

El schema `ceremony_design_schema` enumera exactamente estas siete clases. Un
objeto `pattern` usa `kind`, `roles` e `instructions`; los límites adicionales
que siguen son condiciones del schema o de los fragments canónicos.

| Arquitectura | Elegir cuando | Límite principal |
|---|---|---|
| `sequential` | El orden y el contexto acumulado importan | No selecciona speakers dinámicamente |
| `concurrent` | Hay trabajo independiente y un join claro | Capacidad efectiva = mínimo entre `max_parallel` de la definición y el techo del host |
| `broadcast_collect` | Todos analizan el mismo input y una síntesis reúne resultados | Requiere collector/manager y espera de hermanos antes de agregar |
| `group_chat` | Un manager decide speaker y el diálogo tiene turnos acotados | Requiere `manager_role_id`, `max_iterations` y `fallback_role_id` |
| `maker_checker` | Un productor necesita revisión adversarial acotada | El schema limita la lista a dos roles |
| `handoff` | La resolución pasa entre especialidades o a una persona | Requiere al menos dos roles y un límite de rebotes |
| `magentic` | Hay un ledger de tareas y trabajadores dinámicos | Requiere manager, límite de iteraciones y fallback ante atasco |

### `sequential`

Elegir para una cadena fija: preparar → analizar → redactar. Cada etapa puede
ver el output previo si se configura `see_prior`. No hay fan-out automático ni
selección dinámica de speakers.

```json
{"id":"review_flow","pattern":{"kind":"sequential","roles":["LEAD","REVIEWER"],"instructions":"Ejecutar las etapas en orden y conservar el contexto."}}
```

### `concurrent`

Elegir para inspecciones independientes. El engine publica trabajo claimable y
un join (`all_steps_completed`, `any_step_completed` o `steps_completed` con
count) puede habilitar la transición; el host decide cómo ejecutar claims.

```json
{"id":"parallel_checks","pattern":{"kind":"concurrent","roles":["API","DATA","SECURITY"],"instructions":"Analizar la misma entrada de forma independiente.","join":{"condition":"all_steps_completed"}}}
```

No confundas `max_parallel` con procesos creados: el host puede aplicar un
techo menor y MADE no crea workers.

### `broadcast_collect`

Elegir para fan-out idéntico seguido de una síntesis. El fragment canonical
usa un estado concurrente de broadcast y un estado collector con aggregate
`synthesize`; el collector ve los resultados previos.

```json
{"id":"incident_review","pattern":{"kind":"broadcast_collect","roles":["OPS","SECURITY","PRODUCT"],"manager_role_id":"LEAD","instructions":"Analizar independientemente y sintetizar una recomendación."}}
```

El patrón no garantiza tres agentes reales: roles, handlers y proveedores
deben existir en el host.

### `group_chat`

Elegir para diálogo dirigido por un manager. El fragment usa pasos de gestión,
selección y habla; el selector escribe `next_speaker` en contexto y sólo puede
resolver roles permitidos. `max_iterations` y `fallback_role_id` acotan el
diálogo.

```json
{"id":"design_debate","pattern":{"kind":"group_chat","roles":["LEAD","OPS","SECURITY"],"manager_role_id":"LEAD","max_iterations":4,"fallback_role_id":"HUMAN","instructions":"El manager selecciona el siguiente speaker y detiene o deriva el debate."}}
```

### `maker_checker`

Elegir para producir una propuesta y someterla a una segunda especialidad.
El schema exige exactamente dos roles en la composición y un fallback y tope
de iteraciones.

```json
{"id":"change_review","pattern":{"kind":"maker_checker","roles":["MAKER","CHECKER"],"max_iterations":2,"fallback_role_id":"HUMAN","instructions":"El maker propone; el checker acepta o devuelve una revisión acotada."}}
```

No lo uses para una votación de cuatro especialidades: usa fan-out y síntesis.

### `handoff`

Elegir cuando cada rol puede resolver o entregar explícitamente al siguiente.
El fragment canónico usa `handoff_to`, `resolved`, salida humana y
`max_bounces`.

```json
{"id":"ops_security_handoff","pattern":{"kind":"handoff","roles":["OPS","SECURITY","HUMAN"],"max_iterations":3,"fallback_role_id":"HUMAN","instructions":"Resolver o devolver handoff_to al siguiente rol; parar en el límite."}}
```

El handoff no ejecuta por sí mismo la acción externa; sólo coordina la ruta y
la salida registrada.

### `magentic`

Elegir cuando el manager mantiene un ledger de tareas y el trabajo puede
repartirse dinámicamente. El fragment usa un `ledger` escrito en contexto,
workers que devuelven estado y un fallback cuando el trabajo se atasca.

```json
{"id":"research_ledger","pattern":{"kind":"magentic","roles":["MANAGER","WORKER","HUMAN"],"manager_role_id":"MANAGER","max_iterations":5,"fallback_role_id":"HUMAN","instructions":"Crear ledger, escoger tareas abiertas, marcar progreso y derivar atascos."}}
```

El ledger no es un scheduler de procesos ni inventa trabajadores. La anchura
real, el proveedor y la ejecución siguen siendo del host.

### Componer patrones con hijos

Cuando una etapa necesita subceremonias durables, se publican primero las
definiciones hijas y se declara `spawn` en el paso. MADE sella el plan, abre
cada hijo una vez y permite guards `children_completed:<step>:all`, `:any` o
`:quorum:<n>`. El hijo debe terminar con un `CeremonyCompleted` verificable.
El patrón de coordinación y el uso de hijos son decisiones separadas.

## 9. Pendientes explícitos y límites de afirmación

- **Pendiente de verificar:** registro efectivo de este binario en un host
  concreto (Codex, Claude u otro), disponibilidad de su UI y ciclo de
  reinicio del proceso anfitrión.
- **Pendiente de proveer:** handlers reales, fuentes `source_id`, agentes,
  modelos, credenciales, cuotas y calidad de resultados de proveedores.
- **Pendiente de autorizar:** grants/scopes, aprobación humana, mutaciones de
  repositorio, despliegues, publicaciones externas y acciones de incidente.
- **No afirmado:** creación automática de subagentes/procesos, daemon de
  workers implícito, sandbox equivalente en todos los sistemas o garantía
  exactly-once para cualquier sistema externo.
- **No confundir:** una fixture o un handler noop prueba cableado y contrato,
  no calidad de modelo ni seguridad de producción.

## 10. Próximo paso de aceptación

Seguir el [checklist de aceptación](checklist-aceptacion.md) en una instalación
identificada, guardando el backend, versión, commit, proveedor/fixture y
permisos usados. La aceptación debe probar el recorrido y los límites; no
convierte pendientes de host/proveedor en capacidades observadas.
