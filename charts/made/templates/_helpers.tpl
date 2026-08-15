{{/*
MADE chart helpers.
*/}}

{{- define "made.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "made.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- $name := default .Chart.Name .Values.nameOverride -}}
{{- if contains $name .Release.Name -}}
{{- .Release.Name | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{- define "made.labels" -}}
app.kubernetes.io/name: {{ include "made.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
app.kubernetes.io/part-of: underpass
{{- end -}}

{{- define "made.selectorLabels" -}}
app.kubernetes.io/name: {{ include "made.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{- define "made.serviceAccountName" -}}
{{- if .Values.serviceAccount.create -}}
{{- default (include "made.fullname" .) .Values.serviceAccount.name -}}
{{- else -}}
{{- default "default" .Values.serviceAccount.name -}}
{{- end -}}
{{- end -}}

{{/*
NATS bus name. MADE treats its event bus as a release-local
component (mirrors KMP's pattern, where every plane owns its own NATS).
Templates and the chart's client URL default to this name when the
embedded NATS is on.
*/}}
{{- define "made.natsFullname" -}}
{{- printf "%s-nats" (include "made.fullname" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "made.natsSelectorLabels" -}}
app.kubernetes.io/name: {{ include "made.natsFullname" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/component: nats
{{- end -}}

{{- define "made.natsLabels" -}}
{{ include "made.natsSelectorLabels" . }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
app.kubernetes.io/part-of: underpass
{{- end -}}

{{/*
Resolve the NATS URL the binary should connect to. When the embedded
server is enabled and `messaging.nats.url` is not overridden, this
points at the release-local Service. Otherwise the value from
`messaging.nats.url` wins. Fails fast if `messaging.nats.enabled` is
true and neither side provides a URL.
*/}}
{{- define "made.natsUrl" -}}
{{- if .Values.messaging.nats.url -}}
{{- .Values.messaging.nats.url -}}
{{- else if .Values.messaging.nats.embedded.enabled -}}
nats://{{ include "made.natsFullname" . }}:4222
{{- else -}}
{{- fail "messaging.nats.enabled=true but no URL: set messaging.nats.url or messaging.nats.embedded.enabled=true" -}}
{{- end -}}
{{- end -}}

{{/*
Image reference. Enforces an explicit `tag` or `digest` unless the
development.allowMutableImageTags escape hatch is set.
*/}}
{{- define "made.image" -}}
{{- $repo := required "image.repository is required" .Values.image.repository -}}
{{- if .Values.image.digest -}}
{{ $repo }}@{{ .Values.image.digest }}
{{- else if .Values.image.tag -}}
{{- if and (eq .Values.image.tag "latest") (not .Values.development.allowMutableImageTags) -}}
{{- fail "image.tag=\"latest\" is a mutable reference; set image.tag or image.digest to a pinned reference, or enable development.allowMutableImageTags for non-production use" -}}
{{- end -}}
{{ $repo }}:{{ .Values.image.tag }}
{{- else -}}
{{- fail "set image.tag or image.digest to a pinned reference" -}}
{{- end -}}
{{- end -}}
