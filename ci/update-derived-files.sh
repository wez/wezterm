#!/bin/bash

# Update files that are derived from things baked into the executable

for shell in bash zsh fish ; do
  target/debug/wezterm shell-completion --shell $shell > assets/shell-completion/$shell
done

for mode in copy_mode search_mode ; do
  fname="docs/examples/default-$(echo $mode | tr _ -)-key-table.markdown"
  # Make a wrapped up version of this as markdown, as
  # gelatyx doesn't understand the file include mechanism
  # when used in a lua block
  echo "\`\`\`lua" > $fname
  target/debug/wezterm -n show-keys --lua --key-table $mode >> $fname
  echo "\`\`\`" >> $fname
done

cargo run --example narrow $PWD/target/debug/wezterm --help | ./target/debug/strip-ansi-escapes > docs/examples/cmd-synopsis-wezterm--help.txt

for cmd in start ssh serial connect ls-fonts show-keys imgcat set-working-directory record replay  ; do
  fname="docs/examples/cmd-synopsis-wezterm-${cmd}--help.txt"
  cargo run --example narrow $PWD/target/debug/wezterm $cmd --help | ./target/debug/strip-ansi-escapes > $fname
done

for cmd in \
    activate-pane \
    activate-pane-direction \
    activate-tab \
    get-text \
    kill-pane \
    list \
    list-clients \
    move-pane-to-new-tab \
    rename-workspace \
    send-text \
    set-tab-title \
    set-window-title \
    spawn \
    split-pane \
    ; do
  fname="docs/examples/cmd-synopsis-wezterm-cli-${cmd}--help.txt"
  cargo run --example narrow $PWD/target/debug/wezterm cli $cmd --help | ./target/debug/strip-ansi-escapes > $fname
done
