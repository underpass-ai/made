# C6.10 — Identidad, claves y transición

MADE conserva la procedencia histórica y no reescribe journals durante una
rotación. La autoridad actual sigue siendo el principal autenticado, el grant,
la acción y el target exacto de la operación. La consola no envía el principal
como metadata; en mTLS se deriva de la identidad presentada por el servidor.

## Procedimiento operativo

1. Publicar la nueva configuración con la clave/certificado de verificación
   anterior todavía habilitada y la nueva como activa.
2. Verificar que una réplica con configuración vieja sólo puede leer lo que su
   grant vigente permite; no puede usar la transición para saltarse revocación,
   expiración, fence o drenaje.
3. Renovar clientes y hosts, comprobar fingerprints y registrar la decisión
   de administración fuera de cualquier secreto.
4. Retirar la clave antigua cuando no queden escritores con la versión vieja.
   La retirada de una clave de cursor puede invalidar cursores; eso no revoca
   una autorización histórica ni altera un recibo.

Los PEM, tokens y valores secretos no se guardan en recibos, manifests,
artefactos ni logs. Los checks de esta superficie son dirigidos: fingerprint,
principal/grant, revocación, transición con configuración incompleta y rechazo
de metadata suplantada. MADE no pretende ser un IAM federado.
