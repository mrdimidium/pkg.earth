#!/bin/sh
# SPDX-FileCopyrightText: 2026 Nikolay Govorov
# SPDX-License-Identifier: MPL-2.0
set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
chart=$root/deploy/charts/pkg-earth

helm() {
    mise x helm@4.1.1 -- helm "$@"
}

mise run chart -- --chart "$chart" --lint-only

rendered=$(mktemp)
external=$(mktemp)
routed=$(mktemp)
customized=$(mktemp)
metadata=$(mktemp -d)
trap 'rm -f "$rendered" "$external" "$routed" "$customized"; rm -rf "$metadata"' EXIT

helm template pkg-earth "$chart" >"$rendered"
grep -q '^kind: Deployment$' "$rendered"
grep -q '^kind: Service$' "$rendered"
grep -q '^kind: ConfigMap$' "$rendered"
grep -q 'image: "ghcr.io/mrdimidium/pkg.earth:dev"' "$rendered"
grep -q 'mountPath: /etc/pkg-earth/pkg-earth.toml' "$rendered"
grep -q 'containerPort: 2000' "$rendered"
grep -q 'addr = "0.0.0.0:2000"' "$rendered"
if grep -q '^kind: HTTPRoute$' "$rendered"; then
    echo 'chart rendered HTTPRoute while route.enabled=false' >&2
    exit 1
fi

cat >"$customized" <<'EOF'
serviceAccountName: pkg-earth-runtime
extraVolumes:
  - name: workload-identity
    secret:
      secretName: workload-identity
extraVolumeMounts:
  - name: workload-identity
    mountPath: /var/run/workload-identity
    readOnly: true
EOF
helm template pkg-earth "$chart" --values "$customized" >"$rendered"
grep -q 'serviceAccountName: "pkg-earth-runtime"' "$rendered"
grep -q 'secretName: workload-identity' "$rendered"
grep -q 'mountPath: /var/run/workload-identity' "$rendered"

helm template pkg-earth "$chart" \
    --set config.existingConfigMap=pkg-earth-runtime >"$external"
grep -q 'name: pkg-earth-runtime' "$external"
if grep -q '^kind: ConfigMap$' "$external"; then
    echo 'chart rendered a ConfigMap when an existing one was selected' >&2
    exit 1
fi

helm template pkg-earth "$chart" \
    --set route.enabled=true \
    --set 'route.hostnames[0]=pkg-earth.example.test' \
    --set 'route.parentRefs[0].name=public' >"$routed"
grep -q '^kind: HTTPRoute$' "$routed"
grep -q -- '- pkg-earth.example.test' "$routed"

cp -R "$chart/." "$metadata/"
sed -i 's/^appVersion:.*/appVersion: "1.2.3+metadata"/' "$metadata/Chart.yaml"
helm template pkg-earth "$metadata" >"$rendered"
grep -q 'image: "ghcr.io/mrdimidium/pkg.earth:1.2.3_metadata"' "$rendered"
grep -q 'app.kubernetes.io/version: "1.2.3_metadata"' "$rendered"
