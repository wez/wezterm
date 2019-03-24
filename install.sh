#!/bin/bash

case $OSTYPE in
  darwin*)
    cargo build --release
    mkdir -p $HOME/Applications
    APP=$HOME/Applications/WezTerm.app
    rm -rf $APP
    cp -r assets/macos/WezTerm.app $APP && \
      cp target/release/wezterm $APP && \
        echo "Installed to $APP"
    ;;
  *)
    echo "Don't know how to install the app on this system"
    exit 1
    ;;
esac
