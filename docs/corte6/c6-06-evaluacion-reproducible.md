# C6.6 — Evaluación reproducible

El contrato local vive en `scripts/evaluation/`. El corpus se define en
`corte6-corpus.toml` y se expande a 100 casos deterministas en tres familias:
cambios de software, diagnóstico de incidentes e investigación con evidencia.
La especificación es deliberadamente independiente del proveedor.

Validar el corpus y ver las entradas completas:

```bash
python3 scripts/evaluation/corte6_eval.py --print-cases > tmp/corte6-cases.json
```

Ejecutar un adaptador local que lea un caso JSON por stdin y escriba su salida:

```bash
python3 scripts/evaluation/corte6_eval.py \
  --command 'python3 ./mi-adaptador-local.py' \
  --repeat 2 \
  --output tmp/corte6-evaluation.json
```

Cada resultado conserva el identificador del caso, familia, repetición,
hashes de entrada/salida, duración, código de salida y salida acotada. Los
tokens y el coste son `unknown` cuando el adaptador no los reporta; el arnés no
convierte ausencia de medición en cero. La calidad de un proveedor requiere
añadir un oráculo verificable o revisión humana muestreada al recibo; este
fixture sólo prueba el contrato y la procedencia del recorrido.
