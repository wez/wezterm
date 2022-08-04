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
