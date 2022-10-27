#!/bin/bash
set -eux

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
pushd $DIR/../gravity-info-server
set +e
rm ../scripts/gravity-info-server
set -e
cargo build --release
cp target/release/gravity-info-server ../scripts
popd

pushd $DIR/../gravity-info-dash
yarn run build
rm -rf ../scripts/gravity-info-dash/
mkdir ../scripts/gravity-info-dash
cp -r build/* ../scripts/gravity-info-dash
popd

pushd $DIR
ansible-playbook -i hosts  deploy-info-server.yml
popd

