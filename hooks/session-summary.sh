#!/bin/sh
# mycelium-hook-version: 1
# Session summary Stop hook for Claude Code.
# Reads hook JSON from stdin and stores a compact session summary in Hyphae.
# Requires: jq, hyphae

if ! command -v jq >/dev/null 2>&1; then
  exit 0
fi

if ! command -v hyphae >/dev/null 2>&1; then
  exit 0
fi

JQ_CMD=$(command -v jq)
HYPHAE_CMD=$(command -v hyphae)
INPUT=$(cat)

session_id=$(printf '%s' "$INPUT" | "$JQ_CMD" -r '.session_id // .sessionId // empty' 2>/dev/null) || exit 0
transcript_path=$(printf '%s' "$INPUT" | "$JQ_CMD" -r '.transcript_path // .transcriptPath // empty' 2>/dev/null) || exit 0
cwd=$(printf '%s' "$INPUT" | "$JQ_CMD" -r '.cwd // empty' 2>/dev/null) || exit 0

if [ -z "$session_id" ]; then
  exit 0
fi

if [ -z "$cwd" ]; then
  cwd=$(pwd 2>/dev/null || printf '%s' "")
fi

project_root=""
if [ -n "$cwd" ]; then
  if command -v git >/dev/null 2>&1; then
    project_root=$(git -C "$cwd" rev-parse --show-toplevel 2>/dev/null || printf '%s' "")
  fi

  if [ -z "$project_root" ]; then
    current="$cwd"
    while [ -n "$current" ] && [ "$current" != "/" ]; do
      if [ -e "$current/.git" ]; then
        project_root="$current"
        break
      fi
      current=$(dirname "$current")
    done
  fi
fi

if [ -n "$project_root" ]; then
  project=$(basename "$project_root" 2>/dev/null)
else
  project=$(basename "$cwd" 2>/dev/null)
fi

workspace_root="$project_root"
if [ -z "$workspace_root" ]; then
  workspace_root="$cwd"
fi

if [ -n "$transcript_path" ] && [ -r "$transcript_path" ] && [ -n "$workspace_root" ]; then
  # shellcheck disable=SC2016
  workspace_projects=$(
    "$JQ_CMD" -Rr --arg root "$workspace_root" '
      def entries:
        split("\n")
        | map(select(length > 0) | try fromjson catch empty);

      def blocks($entry):
        $entry.message.content? // [];

      def top_level_repo($candidate):
        if ($candidate | type) != "string" or ($candidate | length) == 0 then
          empty
        else
          ($candidate | split("/")[0]) as $first
          | select($first != "" and $first != "." and $first != "..")
          | $first
        end;

      [
        entries[]
        | select(.type == "assistant")
        | blocks(.)[]
        | if .type == "tool_use" and (.name == "Write" or .name == "Edit" or .name == "MultiEdit") then
            .input.file_path? // empty
          elif .type == "tool_use" and .name == "Bash" then
            (.input.command? // empty)
            | capture("(^|[;&|]\\s*)cd\\s+(?<dir>[^\\s;&|]+)")?.dir // empty
          else
            empty
          end
        | if startswith($root + "/") then
            .[($root | length + 1):]
          else
            .
          end
        | top_level_repo(.)
        | select(length > 0)
      ] | .[]
    ' "$transcript_path" 2>/dev/null
  ) || workspace_projects=""

  if [ -n "$workspace_projects" ]; then
    old_ifs=$IFS
    IFS='
'
    for workspace_project in $workspace_projects; do
      if [ -n "$workspace_project" ] && [ -e "$workspace_root/$workspace_project/.git" ]; then
        project="$workspace_project"
        break
      fi
    done
    IFS=$old_ifs
  fi
fi

case "$project" in
  ""|"/")
    project="unknown"
    ;;
esac

if [ -n "$transcript_path" ] && [ -r "$transcript_path" ]; then
  # shellcheck disable=SC2016
  summary=$(
    "$JQ_CMD" -Rsc -r --arg session_id "$session_id" --arg cwd "$cwd" --arg project "$project" '
      def entries:
        split("\n")
        | map(select(length > 0) | try fromjson catch empty);

      def blocks($entry):
        $entry.message.content? // [];

      def as_text:
        if type == "string" then .
        elif type == "array" then
          map(
            if type == "string" then .
            elif type == "object" then (.text? // .content? // "")
            else ""
            end
          ) | join("")
        elif type == "object" then (.text? // .content? // "")
        else ""
        end;

      entries as $entries
      | [$entries[] | select(.type == "assistant") | blocks(.)[]] as $assistant_blocks
      | [$entries[] | select(.type == "user") | blocks(.)[]] as $user_blocks
      | [$assistant_blocks[] | select(.type == "tool_use" and .name == "Bash") | .input.command? | strings] as $commands
      | [$assistant_blocks[] | select(.type == "tool_use" and (.name == "Write" or .name == "Edit" or .name == "MultiEdit")) | .input.file_path? | strings] | unique as $files
      | [$user_blocks[] | select(.type == "tool_result" and (.is_error // false)) | .content | as_text | gsub("\\s+"; " ") | sub("^ "; "") | sub(" $"; "") | select(length > 0)] as $errors
      | [
          "Session summary",
          "Session ID: \($session_id)",
          "Project: \($project)",
          "Working directory: \($cwd)",
          "Transcript messages: \($entries | length)",
          "Commands run: \($commands | length)",
          (
            if ($commands | length) > 0 then
              ($commands[:10] | map("- " + .) | join("\n"))
            else
              "- none"
            end
          ),
          "Files modified: \($files | length)",
          (
            if ($files | length) > 0 then
              ($files[:10] | map("- " + .) | join("\n"))
            else
              "- none"
            end
          ),
          "Errors: \($errors | length)",
          (
            if ($errors | length) > 0 then
              ($errors[:5] | map("- " + .) | join("\n"))
            else
              "- none"
            end
          )
        ] | join("\n")
    ' "$transcript_path" 2>/dev/null
  ) || summary=""
else
  summary=""
fi

if [ -z "$summary" ]; then
  summary=$(printf '%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s' \
    "Session summary" \
    "Session ID: $session_id" \
    "Project: $project" \
    "Working directory: $cwd" \
    "Transcript messages: unavailable" \
    "Commands run: unavailable" \
    "- none" \
    "Files modified: unavailable" \
    "Errors: unavailable" \
    "- none")
fi

(
  "$HYPHAE_CMD" store \
    --topic "session/$project" \
    --content "$summary" \
    --importance medium \
    -P "$project" \
    >/dev/null 2>&1
) &

exit 0
