{{- define "pod-director.name" -}}
  {{- .Values.nameOverride | default .Chart.Name  | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "pod-director.fullname" -}}
  {{- if .Values.fullnameOverride }}
    {{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
  {{- else }}
    {{- $name := .Values.nameOverride | default .Chart.Name  }}
    {{- if .Release.Name | contains $name  }}
      {{- .Release.Name | trunc 63 | trimSuffix "-" }}
    {{- else }}
      {{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
    {{- end }}
  {{- end }}
{{- end }}

{{- define "pod-director.chart" -}}
  {{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "pod-director.labels" -}}
{{- include "pod-director.selectorLabels" . }}
app.kubernetes.io/version: {{ .Chart.Version | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
helm.sh/chart: {{ include "pod-director.chart" . }}
{{- with .Values.globalLabels }}
{{ toYaml . }}
{{- end }}
{{- end }}

{{- define "pod-director.selectorLabels" -}}
app.kubernetes.io/name: {{ include "pod-director.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{- define "pod-director.serviceAccountName" -}}
  {{- .Values.serviceAccount.name | default (include "pod-director.fullname" .)  }}
{{- end }}

{{- define "pod-director.imageFullname" -}}
  {{- $image := .Values.image }}
  {{- $registry := $image.registry }}
  {{- $repository := required "An repository is required" $image.repository }}
  {{- $tag := $image.tag | default .Chart.Version }}
  {{- if $registry }}
    {{- $registry = print $registry "/" }}
  {{- end }}
  {{- if $tag }}
    {{- $tag = print ":" $tag }}
  {{- end }}
  {{- print $registry $repository $tag }}
{{- end }}

{{- define "pod-director.configMapName" -}}
  {{- include "pod-director.fullname" . }}
{{- end }}

{{- define "pod-director.serviceName" -}}
  {{- include "pod-director.fullname" . }}
{{- end }}

{{- define "pod-director.webhookSecretName" -}}
  {{- include "pod-director.fullname" . }}
{{- end }}

{{- define "pod-director.clusterRoleName" -}}
  {{- include "pod-director.fullname" . }}
{{- end }}
