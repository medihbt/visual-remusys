#!/bin/bash

project_dir=$(dirname "$(readlink -f "$0")")
cd "$project_dir"

# Build the web application
pushd remusys-lens
# remove `remusys-wasm` dependency from package.json
jq 'del(.dependencies["remusys-wasm"])' package.json > tmp.json && mv tmp.json package.json
npm install
npm run wasm-build
npm run wasm-refresh
npm run build
popd
