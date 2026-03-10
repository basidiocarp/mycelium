#!/bin/bash
# mycelium-economics.sh
# Combine ccusage (tokens spent) with mycelium (tokens saved) for economic analysis

set -euo pipefail

# =========================================================
#  Colors
# =========================================================
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# =========================================================
#  Header
# =========================================================
CURRENT_MONTH=$(date +%Y-%m)

echo -e "${BLUE}📊 Mycelium Economic Impact Analysis${NC}"
echo "════════════════════════════════════════════════════════════════"
echo

# =========================================================
#  Dependency checks
# =========================================================
if ! command -v ccusage &> /dev/null; then
    echo -e "${RED}Error: ccusage not found${NC}"
    echo "Install: npm install -g @anthropics/claude-code-usage"
    exit 1
fi

if ! command -v mycelium &> /dev/null; then
    echo -e "${RED}Error: mycelium not found${NC}"
    echo "Install: cargo install --path ."
    exit 1
fi

# =========================================================
#  Fetch data
# =========================================================
echo -e "${YELLOW}Fetching token usage data from ccusage...${NC}"
if ! ccusage_json=$(ccusage monthly --json 2>/dev/null); then
    echo -e "${RED}Failed to fetch ccusage data${NC}"
    exit 1
fi

echo -e "${YELLOW}Fetching token savings data from mycelium...${NC}"
if ! mycelium_json=$(mycelium gain --monthly --format json 2>/dev/null); then
    echo -e "${RED}Failed to fetch mycelium data${NC}"
    exit 1
fi

echo

# =========================================================
#  Parse data
# =========================================================
ccusage_cost=$(echo "$ccusage_json" | jq -r ".monthly[] | select(.month == \"$CURRENT_MONTH\") | .totalCost // 0")
ccusage_input=$(echo "$ccusage_json" | jq -r ".monthly[] | select(.month == \"$CURRENT_MONTH\") | .inputTokens // 0")
ccusage_output=$(echo "$ccusage_json" | jq -r ".monthly[] | select(.month == \"$CURRENT_MONTH\") | .outputTokens // 0")
ccusage_total=$(echo "$ccusage_json" | jq -r ".monthly[] | select(.month == \"$CURRENT_MONTH\") | .totalTokens // 0")

mycelium_saved=$(echo "$mycelium_json" | jq -r ".monthly[] | select(.month == \"$CURRENT_MONTH\") | .saved_tokens // 0")
mycelium_commands=$(echo "$mycelium_json" | jq -r ".monthly[] | select(.month == \"$CURRENT_MONTH\") | .commands // 0")
mycelium_input=$(echo "$mycelium_json" | jq -r ".monthly[] | select(.month == \"$CURRENT_MONTH\") | .input_tokens // 0")
mycelium_output=$(echo "$mycelium_json" | jq -r ".monthly[] | select(.month == \"$CURRENT_MONTH\") | .output_tokens // 0")
mycelium_pct=$(echo "$mycelium_json" | jq -r ".monthly[] | select(.month == \"$CURRENT_MONTH\") | .savings_pct // 0")

# =========================================================
#  Calculate economics
# =========================================================
saved_cost=$(echo "scale=2; $mycelium_saved * 0.0001" | bc 2>/dev/null || echo "0")
total_without_mycelium=$(echo "scale=2; $ccusage_cost + $saved_cost" | bc 2>/dev/null || echo "$ccusage_cost")

if (( $(echo "$total_without_mycelium > 0" | bc -l) )); then
    savings_pct=$(echo "scale=1; ($saved_cost / $total_without_mycelium) * 100" | bc 2>/dev/null || echo "0")
else
    savings_pct="0"
fi

if [ "$mycelium_commands" -gt 0 ]; then
    cost_per_cmd_with=$(echo "scale=2; $ccusage_cost / $mycelium_commands" | bc 2>/dev/null || echo "0")
    cost_per_cmd_without=$(echo "scale=2; $total_without_mycelium / $mycelium_commands" | bc 2>/dev/null || echo "0")
else
    cost_per_cmd_with="N/A"
    cost_per_cmd_without="N/A"
fi

format_number() {
    local num=$1
    if [ "$num" = "0" ] || [ "$num" = "N/A" ]; then
        echo "$num"
    else
        echo "$num" | numfmt --to=si 2>/dev/null || echo "$num"
    fi
}

# =========================================================
#  Report
# =========================================================
cat << EOF
${GREEN}💰 Economic Impact Report - $CURRENT_MONTH${NC}
════════════════════════════════════════════════════════════════

${BLUE}Tokens Consumed (via Claude API):${NC}
  Input tokens:        $(format_number $ccusage_input)
  Output tokens:       $(format_number $ccusage_output)
  Total tokens:        $(format_number $ccusage_total)
  ${RED}Actual cost:         \$$ccusage_cost${NC}

${BLUE}Tokens Saved by mycelium:${NC}
  Commands executed:   $mycelium_commands
  Input avoided:       $(format_number $mycelium_input) tokens
  Output generated:    $(format_number $mycelium_output) tokens
  Total saved:         $(format_number $mycelium_saved) tokens (${mycelium_pct}% reduction)
  ${GREEN}Cost avoided:        ~\$$saved_cost${NC}

${BLUE}Economic Analysis:${NC}
  Cost without mycelium:    \$$total_without_mycelium (estimated)
  Cost with mycelium:       \$$ccusage_cost (actual)
  ${GREEN}Net savings:         \$$saved_cost ($savings_pct%)${NC}
  ROI:                 ${GREEN}Infinite${NC} (mycelium is free)

${BLUE}Efficiency Metrics:${NC}
  Cost per command:    \$$cost_per_cmd_without → \$$cost_per_cmd_with
  Tokens per command:  $(echo "scale=0; $mycelium_input / $mycelium_commands" | bc 2>/dev/null || echo "N/A") → $(echo "scale=0; $mycelium_output / $mycelium_commands" | bc 2>/dev/null || echo "N/A")

${BLUE}12-Month Projection:${NC}
  Annual savings:      ~\$$(echo "scale=2; $saved_cost * 12" | bc 2>/dev/null || echo "0")
  Commands needed:     $(echo "$mycelium_commands * 12" | bc 2>/dev/null || echo "0") (at current rate)

════════════════════════════════════════════════════════════════

${YELLOW}Note:${NC} Cost estimates use \$0.0001/token average. Actual pricing varies by model.
See ccusage for precise model-specific costs.

${GREEN}Recommendation:${NC} Focus mycelium usage on high-frequency commands (git, grep, ls)
for maximum cost reduction.

EOF
