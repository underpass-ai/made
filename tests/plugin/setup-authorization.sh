#!/usr/bin/env bash
# Source in an isolated CI fixture after setting MADE_MCP_STORE_PATH.
# These identities and the public test key are never operator defaults.
export MADE_AUTH_POLICY_ID=plugin-ci-policy
export MADE_AUTH_TRUSTED_HOST_ID=plugin-ci-host
export MADE_CEREMONY_STORE_ID=plugin-ci-store
export MADE_CEREMONY_SEARCH_CURSOR_HMAC_KEY=a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5
"$1" bootstrap-authorization "${MADE_MCP_STORE_PATH:?isolated CI store required}" \
  --policy-id "${MADE_AUTH_POLICY_ID}" --trusted-host-id "${MADE_AUTH_TRUSTED_HOST_ID}"
