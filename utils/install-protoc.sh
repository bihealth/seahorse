#!/usr/bin/bash

# Will install into ~/.local/share/protoc, so make sure to add the following
# to your PATH: ~/.local/share/protoc/bin
#
# Will go into ./utils/var for cloning/building.

mkdir -p utils/var
cd utils/var

apt-get install git cmake build-essentials

if [[ ! -e protobuf ]]; then
    git clone https://github.com/protocolbuffers/protobuf.git
fi
cd protobuf
git submodule update --init --recursive

cmake . -DCMAKE_INSTALL_PREFIX=$HOME/.local/share/protoc
make -j 8 install

mkdir -p ~/.local/share/protoc/bin
cp bazel-bin/protoc ~/.local/share/protoc/bin
