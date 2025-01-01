#!/bin/zsh

# Save initial unicode version
local label=$(uuidgen)
printf "\e]1337;UnicodeVersion=push %s\e\\" $label

for version in 9 14 ; do
  echo "\n### Unicode Version $version\n"
  printf "\e]1337;UnicodeVersion=$version\e\\"

  echo "|||"
  echo -e "\u270c|  Victory hand, text presentation by default"
  echo -e "\u270c\ufe0e|  Victory hand, explicit text presentation"
  echo -e "\u270c\ufe0f|  Victory hand, explicit emoji presentation"

  echo -e "\u270a|  Raised fist, emoji presentation by default"
  echo -e "\u270a\ufe0e|  Raised fist, explicit text presentation (invalid; no effect)"
  echo -e "\u270a\ufe0f|  Raised fist, explicit emoji presentation"

  echo -e "\u2716|  Multiply, text presentation by default"
  echo -e "\u2716\ufe0e|  Multiply, explicit text presentation"
  echo -e "\u2716\ufe0f|  Multiply, explicit emoji presentation"

  echo -e "\U0001F468\u200D\U0001F467\u200D\U0001F466|"
  echo "👨|"
  echo -e "\U0001F6E5|"
  echo -e "\U0001F6E5"
done

# Restore saved unicode version
printf "\e]1337;UnicodeVersion=pop %s\e\\" $label
