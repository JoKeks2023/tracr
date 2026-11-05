#!/usr/bin/env bash
# Fails if a tracked file contains a real user's home path or an e-mail
# address. Placeholders used by tests and docs are allowed below; add a new
# one only when it is clearly not a real person.
#
# Run locally from the repository root: .github/scripts/check-personal-data.sh
set -euo pipefail

# Home directory names that are placeholders or CI runners, not people.
home_allow='^(/Users|/home)/(dev|you|user|username|name|me|someone|somebody|example|fixture|x|ada|runner|build|test|tester|alice|bob)/$'

# E-mail addresses that are placeholders, no-reply or not a mailbox at all.
mail_allow='(^noreply@anthropic\.com$|@users\.noreply\.github\.com$|@github\.com$|@([a-z0-9-]+\.)*example\.(com|org|net)$|\.(png|jpg|jpeg|gif|svg|webp|ico)$)'

status=0

homes=$(git grep -n -I -o -E '(/Users|/home)/[A-Za-z0-9._-]+/' -- . ':!.github/scripts/check-personal-data.sh' || true)
bad_homes=$(printf '%s\n' "$homes" | awk -F: 'NF >= 3 { print }' | while IFS= read -r line; do
  path=${line#*:*:}
  if ! printf '%s\n' "$path" | grep -q -E "$home_allow"; then
    printf '%s\n' "$line"
  fi
done)
if [ -n "$bad_homes" ]; then
  echo "::error::Tracked files contain a home directory path of a real user:"
  printf '%s\n' "$bad_homes"
  status=1
fi

mails=$(git grep -n -I -o -E '[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}' -- . ':!.github/scripts/check-personal-data.sh' || true)
bad_mails=$(printf '%s\n' "$mails" | awk -F: 'NF >= 3 { print }' | while IFS= read -r line; do
  addr=${line#*:*:}
  if ! printf '%s\n' "$addr" | grep -q -i -E "$mail_allow"; then
    printf '%s\n' "$line"
  fi
done)
if [ -n "$bad_mails" ]; then
  echo "::error::Tracked files contain an e-mail address:"
  printf '%s\n' "$bad_mails"
  status=1
fi

if [ "$status" -eq 0 ]; then
  echo "No home paths or e-mail addresses of real people in tracked files."
fi
exit "$status"
