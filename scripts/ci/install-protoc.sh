#!/usr/bin/env bash
set -euo pipefail

if command -v protoc >/dev/null 2>&1; then
  protoc --version
  exit 0
fi

if command -v apt-get >/dev/null 2>&1; then
  export DEBIAN_FRONTEND=noninteractive
  if command -v sudo >/dev/null 2>&1; then
    sudo apt-get update
    sudo apt-get install -y protobuf-compiler
  else
    apt-get update
    apt-get install -y protobuf-compiler
  fi
elif command -v brew >/dev/null 2>&1; then
  brew install protobuf
elif command -v choco >/dev/null 2>&1; then
  choco install protoc --no-progress -y
  hash -r
else
  echo "protoc is required but apt-get, brew and choco are unavailable" >&2
  exit 1
fi

protoc --version
