#!/bin/bash
set -x
set -e

max_attempts=4
attempt=1

until "$@"
do
  if (( attempt == max_attempts ))
  then
    echo "Failed after $max_attempts attempts"
    exit 1
  fi

  sleep 5
  : $(( attempt++ ))
done
