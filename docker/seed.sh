#!/usr/bin/env bash
# Firestore 에뮬레이터에 검증용 샘플 데이터를 주입한다.
# 에뮬레이터는 `Authorization: Bearer owner`를 admin으로 취급해
# firestore.rules(allow write: if false)를 우회한다.
#
# 사용: docker 에뮬레이터가 healthy인 상태에서
#   bash docker/seed.sh
set -euo pipefail

HOST="${FIRESTORE_EMULATOR_HOST:-localhost:8080}"
PROJECT="${GCLOUD_PROJECT:-demo-firescope}"
BASE="http://${HOST}/v1/projects/${PROJECT}/databases/(default)/documents"

put() { # collection id json-fields
  curl -s -o /dev/null -w "%{http_code}" \
    -X POST "${BASE}/$1?documentId=$2" \
    -H "Authorization: Bearer owner" \
    -H "Content-Type: application/json" \
    -d "$3"
}

echo "seeding users (150) ..."
for i in $(seq 1 150); do
  id=$(printf "u%03d" "$i")
  active=$([ $((i % 3)) -eq 0 ] && echo false || echo true)
  body=$(cat <<JSON
{"fields":{
  "name":{"stringValue":"User ${i}"},
  "age":{"integerValue":"$((18 + i % 50))"},
  "active":{"booleanValue":${active}},
  "profile":{"mapValue":{"fields":{"city":{"stringValue":"Seoul"},"score":{"doubleValue":$((i % 100)).5}}}},
  "tags":{"arrayValue":{"values":[{"stringValue":"t${i}"},{"stringValue":"common"}]}}
}}
JSON
)
  code=$(put users "$id" "$body")
  [ "$code" = "200" ] || { echo "FAIL user $id -> HTTP $code"; exit 1; }
done

echo "seeding posts (5) ..."
for i in $(seq 1 5); do
  put posts "$(printf "p%02d" "$i")" \
    "{\"fields\":{\"title\":{\"stringValue\":\"Post ${i}\"},\"views\":{\"integerValue\":\"$((i * 10))\"}}}" \
    >/dev/null
done

echo "done. collections: users(150), posts(5)"
