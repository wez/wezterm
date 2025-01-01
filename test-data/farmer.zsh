#!/bin/zsh

man=$(echo -e "\U0001F468")
ear_of_rice=$(echo -e "\U0001F33E")
farmer=$(echo -e "${man}\u200D${ear_of_rice}")

echo -e "$farmer$farmer$farmer."
echo -e "$farmer\b\b$farmer\b\b$farmer."
echo -e "$farmer\b$farmer\b$farmer."
echo -e "$farmer\b\b\b\b\b$farmer\b\b\b\b\b$farmer."
echo -ne "$farmer"
sleep 1
echo -e "\b\b\b\b\b."
echo "DONE"
