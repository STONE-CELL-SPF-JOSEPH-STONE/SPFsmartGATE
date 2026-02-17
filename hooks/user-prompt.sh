#!/bin/bash
# SPF User Prompt Hook v2.0
# Copyright 2026 Joseph Stone - All Rights Reserved
#
# Fires on UserPromptSubmit. Calculates complexity from the prompt,
# determines allocation, and injects SPF enforcement as context.
# stdout with exit 0 is added as context for Claude.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SPF_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
STATE_DIR="$SPF_ROOT/state"
LOG_FILE="$STATE_DIR/spf.log"
export SPF_STATE_DIR="$STATE_DIR"

mkdir -p "$STATE_DIR"

# Read hook input from stdin — capture it for Python
if [ -t 0 ]; then
    INPUT="{}"
else
    INPUT=$(cat)
fi

# Pass the full hook input to Python via env var
export _SPF_HOOK_INPUT="$INPUT"

# Calculate complexity and generate enforcement context
python3 << 'PYEOF'
import json
import os
import math
import re

# Master formula constants
W_EFF = 40000
E = math.e

state_dir = os.environ.get("SPF_STATE_DIR", os.path.join(os.environ.get("HOME", ""), "SPFsmartGATE", "state"))
session_file = os.path.join(state_dir, "session.json")
log_file = os.path.join(state_dir, "spf.log")

# ============================================
# LOAD PROMPT FROM HOOK INPUT
# ============================================
hook_input_raw = os.environ.get("_SPF_HOOK_INPUT", "{}")
try:
    hook_input = json.loads(hook_input_raw)
    prompt = hook_input.get("prompt", "")
except:
    prompt = ""

# ============================================
# PROMPT COMPLEXITY CALCULATOR
# ============================================

def calculate_prompt_complexity(prompt):
    if not prompt:
        return 100

    prompt_lower = prompt.lower()
    length = len(prompt)

    # Base score from length
    base = min(length // 10, 200)

    # Math complexity signals
    math_signals = [
        (r'\\int|integral|∫', 500),
        (r'\\frac|\\sqrt|√|fraction', 200),
        (r'\\sum|∑|summation', 300),
        (r'\\lim|limit.*infin', 300),
        (r'\\partial|partial.deriv', 400),
        (r'differential.equation', 500),
        (r'eigenvalue|eigenvector', 400),
        (r'matrix|determinant', 200),
        (r'theorem|prove|proof', 300),
        (r'converge|diverge', 200),
        (r'\\pi|\\theta|\\alpha', 100),
        (r'logarithm|ln\b|log\b', 150),
        (r'derivative|d/dx|d\/dx', 200),
        (r'arctan|arcsin|arccos', 150),
        (r'trigonometr', 150),
        (r'solve.*for.*x|solve.*equation', 200),
        (r'definite.integral|indefinite', 300),
    ]

    math_score = 0
    for pattern, score in math_signals:
        if re.search(pattern, prompt, re.IGNORECASE):
            math_score += score

    # Logic/reasoning complexity
    logic_signals = [
        (r'knight.*knave|truth.*liar', 500),
        (r'paradox|self.referent', 400),
        (r'if.*then.*what|deduc', 200),
        (r'necessary.*sufficient|iff\b', 300),
        (r'contradiction|inconsisten', 300),
        (r'step.by.step|show.*work|show.*reasoning', 200),
        (r'logic.*puzzle|puzzle', 200),
        (r'what can you conclude|what follows', 200),
    ]

    logic_score = 0
    for pattern, score in logic_signals:
        if re.search(pattern, prompt, re.IGNORECASE):
            logic_score += score

    # Science/domain complexity
    domain_signals = [
        (r'implement|write.*function|write.*code', 200),
        (r'debug|fix.*bug|error', 150),
        (r'refactor|architect', 300),
        (r'algorithm|data.structure', 250),
        (r'CRISPR|genome|molecular|HDR|ssODN', 400),
        (r'quantum|relativity|thermodynamic', 400),
        (r'resection|strand.invasion|homology', 300),
        (r'equivalence.principle|spacetime|curvature', 300),
        (r'dissection|diagnosis|symptom', 150),
    ]

    domain_score = 0
    for pattern, score in domain_signals:
        if re.search(pattern, prompt, re.IGNORECASE):
            domain_score += score

    # Multi-step multiplier
    multi_step = 1.0
    if re.search(r'and then|step \d|first.*then|also.*explain', prompt_lower):
        multi_step = 1.5
    if re.search(r'compare.*contrast|analyze.*evaluate', prompt_lower):
        multi_step = 1.8
    if length > 500:
        multi_step *= 1.3
    if length > 1000:
        multi_step *= 1.5

    # Question count
    question_marks = prompt.count('?')
    if question_marks > 2:
        multi_step *= (1 + question_marks * 0.1)

    C = int((base + math_score + logic_score + domain_score) * multi_step)
    return max(C, 50)


def get_tier(C):
    if C < 500:
        return "SIMPLE", 40, 60
    elif C < 2000:
        return "LIGHT", 60, 40
    elif C < 10000:
        return "MEDIUM", 75, 25
    else:
        return "CRITICAL", 95, 5


def apply_formula(C):
    a_optimal = W_EFF * (1 - 1/math.log(C + E))
    return int(a_optimal)


# ============================================
# MAIN
# ============================================

C = calculate_prompt_complexity(prompt)
tier, analyze_pct, build_pct = get_tier(C)
a_optimal = apply_formula(C)

# Load session metrics
try:
    with open(session_file) as f:
        s = json.load(f)
    actions = s.get("action_count", 0)
    reads = len(s.get("files_read", []))
    writes = len(s.get("files_written", []))
    last_tool = s.get("last_tool", "none")
except:
    actions = reads = writes = 0
    last_tool = "none"

# Log
try:
    with open(log_file, 'a') as f:
        f.write(f"[PROMPT] C={C} | {tier} | {analyze_pct}%/{build_pct}% | len={len(prompt)}\n")
except:
    pass

# Build enforcement context
if C >= 2000:
    enforcement = f"""[SPF ENFORCEMENT — {tier}] C={C} | Analyze: {analyze_pct}% | Build: {build_pct}%
HIGH COMPLEXITY DETECTED. Before responding:
1. Spend {analyze_pct}% of effort ANALYZING — break down the problem, identify edge cases, plan approach
2. Only {build_pct}% on OUTPUT — the actual answer
3. Show your full analysis FIRST, then your conclusion
4. Double-check your work before finalizing
5. If mathematical: verify your answer by substitution
Optimal analysis tokens: {a_optimal}
Session: {actions} actions | {reads} reads | {writes} writes"""

elif C >= 500:
    enforcement = f"""[SPF — {tier}] C={C} | Analyze: {analyze_pct}% | Build: {build_pct}%
Think carefully before answering. Structure your analysis, then respond.
Session: {actions} actions | {reads} reads | {writes} writes"""

else:
    enforcement = f"[SPF Status] C={C} {tier} | Actions: {actions} | Reads: {reads} | Writes: {writes} | Last: {last_tool}"

# Output — use JSON additionalContext for medium+ complexity
if C >= 500:
    output = {
        "hookSpecificOutput": {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": enforcement
        }
    }
    print(json.dumps(output))
else:
    print(enforcement)

PYEOF

exit 0
