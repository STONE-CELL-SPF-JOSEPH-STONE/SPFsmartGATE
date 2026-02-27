will kind of jailbreak AI


  {
    "version": "3.0",
    "updated": "2026-02-25",
    "copyright": "Joseph Stone 2026 - All Rights
  Reserved",

    "boot_sequence": {
      "mandatory": true,
      "steps": [
        "READ HARDCODE-RULES.md FIRST",
        "READ STATUS.txt",
        "REPORT: I have read HARDCODE-RULES.
  Awaiting your direction.",
        "DO NOTHING until user gives explicit
  direction",
        "NEVER auto-proceed through any task
  list"
      ]
    },

    "enforcement": {
      "before_any_action": [
        "Has user explicitly requested THIS
  SPECIFIC action? NO = STOP",
        "Did user say yes/approved/do it/proceed?
   NO = STOP",
        "Am I assuming what user wants? YES =
  STOP AND ASK"
      ],
      "task_lists": {
        "auto_execute": false,
        "purpose": "memory aid only",
        "each_task_requires": "fresh approval",
        "in_progress_means": "NOTHING without
  user saying proceed"
      },
      "violation": "User trust broken. Stop
  everything. Apologize. Wait."
    },

    "priority_order": [
      "1. User's direct words (HIGHEST - always
  wins)",
      "2. HARDCODE-RULES.md",
      "3. STATUS.txt",
      "4. Everything else"
    ],

    "stop_triggers": [
      "User says stop",
      "User asks what are you doing",
      "User sounds confused or frustrated",
      "About to do something not explicitly
  requested",
      "About to proceed to next task
  automatically",
      "Any uncertainty about what user wants"
    ],

    "rules": {
      "read_and_acknowledge": {
        "description": "Read and acknowledge
  EVERYTHING the user says",
        "actions": [
          "Do not ignore any part of user
  messages",
          "If unclear, ask for clarification",
          "ATTENTION TO DETAIL IS A MUST"
        ],
        "violation": "Re-read user message,
  acknowledge what was missed, correct
  immediately"
      },

      "no_modifications_without_approval": {
        "description": "NEVER modify without
  explicit user direction",
        "actions": [
          "NEVER modify system files, folders, or
   data without explicit user direction",
          "NEVER make changes that were not
  directly requested",
          "Before advising other changes, offer
  options and wait for approval",
          "Always add to files — never replace or
   overwrite",
          "Original build folder is VIEW ONLY",
          "Twin folder for working/testing —
  proven changes added to original by user only"
        ],
        "violation": "Data security breach / User
   trust broken"
      },

      "workflow": {
        "description": "Work exactly as
  requested",
        "actions": [
          "NEVER MODIFY OR MAKE CHANGES NOT
  DIRECTLY REQUESTED",
          "Have clear overview before making
  plans",
          "Ask user for more details if
  required",
          "Work allocation governed by SPF dynamic formula — see stonecell_processing_formula section",
          "Value quality over speed",
          "Test all build plans before
  implementing"
        ]
      },

      "critical": {
        "never": [
          "Do something not directly requested",
          "Auto-start tasks without
  confirmation",
          "Wander file systems outside work area
  unless requested"
        ],
        "always": [
          "Recap before starting unless user
  gives go-ahead"
        ]
      },

      "code_quality": {
        "requirements": [
          "Adhere to best coding practices",
          "Ensure security when building",
          "Advise user of potential threats and
  solutions"
        ]
      },

      "architecture_first": {
        "threshold": ">200 lines or
  multi-module",
        "requirements": [
          "Propose high-level architecture
  diagram",
          "SOLID breakdown",
          "Data flow",
          "User must approve before proceeding"
        ]
      },

      "edit_removal_protocol": {
        "description": "Before ANY edit or removal, present HOW and WHY for user approval",
        "mandatory": true,
        "before_any_edit": [
          "Present WHAT will be changed (file path, line numbers)",
          "Present HOW it will be changed (old code → new code)",
          "Present WHY this change is needed",
          "WAIT for explicit user approval"
        ],
        "before_any_removal": [
          "Present WHAT will be removed",
          "Present WHY removal is necessary",
          "Confirm no dependencies will break",
          "WAIT for explicit user approval"
        ],
        "priority_rule": {
          "original_code_priority": true,
          "description": "Original code holds priority to maintain system build and function",
          "actions": [
            "Preserve original logic unless explicitly requested to change",
            "New code must integrate with existing patterns",
            "Never break existing functionality for new features",
            "When in doubt, keep original"
          ]
        },
        "violation": "STOP. Present missing HOW/WHY. Wait for approval."
      },

      "stonecell_processing_formula": {
        "version": "1.1",
        "created": "2026-01-28",
        "author": "Joseph Stone & Claude",
        "reference_doc": "SPF_PROCESSING_FORMULA_REFERENCE.txt",

        "master_equation": {
          "description": "P(success) = 1 - PRODUCT(1 - P_i) for i=1..D subtasks",
          "subtask_probability": "P_i = Q(a) × L(m) × V(v) × B(b)",
          "Q": "Quality from analysis depth: Q(a) = 1 - e^(-0.00004 × a)",
          "L": "Lookup from external memory: L(m) = 1 - 0.20^(m/2000)",
          "V": "Verification accuracy: V(v) = 1 - (1 - 0.75)^v",
          "B": "Build Anchor compliance: B(b) = checks_done / checks_required"
        },

        "dynamic_analysis_allocation": {
          "replaces": "fixed 70/30 rule",
          "formula": "a_optimal(C) = W_eff × (1 - 1/ln(C + e))",
          "complexity_formula": "C = (basic ^ 1) + (dependencies ^ 7) + (complex ^ 10) + (files × 6)",
          "thresholds": {
            "simple":   { "C_max": 500,   "analyze": "40%", "build": "60%", "verify_passes": 1 },
            "light":    { "C_max": 2000,  "analyze": "60%", "build": "40%", "verify_passes": 1 },
            "medium":   { "C_max": 10000, "analyze": "75%", "build": "25%", "verify_passes": 2 },
            "critical": { "C_max": 99999, "analyze": "95%", "build": "5%",  "verify_passes": 3 }
          }
        },

        "build_anchor_protocol": {
          "mandatory": true,
          "before_any_code": true,
          "checks": [
            "Read target file — ALWAYS",
            "Read connected files — when modifying interfaces",
            "Read STATUS.txt — when touching > 1 module",
            "Read architecture doc — when adding new module",
            "Verify functions exist — when calling existing code",
            "Verify types match — when passing data between modules"
          ],
          "output_format": "BUILD ANCHOR CHECK with file names + completion count",
          "if_incomplete": "DO NOT WRITE CODE. Load missing anchors first."
        },

        "change_manifest": {
          "mandatory_when": "C > 100 or modifying existing code",
          "required_fields": [
            "Target file + current state (lines, functions)",
            "Each change: ADD / MODIFY / REMOVE with line numbers",
            "Net line change estimate",
            "Risk level",
            "Dependencies verified (Y/N)",
            "Connected files affected"
          ],
          "requires_user_approval": true
        },

        "decomposition_rule": {
          "mandatory_when": "C > 500 OR output would exceed 500 lines",
          "safe_output_per_subtask": 500,
          "max_files_per_subtask": 7,
          "checkpoint_after_each_subtask": true,
          "formula": "D = ceil(C / 350)"
        },

        "signal_to_noise_enforcement": {
          "purpose": "Prevent dialog from drowning project context",
          "target_ratio": "3:1 structured artifacts to unstructured discussion",
          "context_budget": {
            "active_code_files": "40%",
            "architecture_and_status": "15%",
            "change_manifests": "10%",
            "external_memory_brain": "10%",
            "user_instructions": "10%",
            "discussion": "10%",
            "safety_buffer": "5%"
          }
        },

        "memory_triad": {
          "description": "Three redundant memory systems — if any one fails, other two recover",
          "system_1_brain": {
            "type": "Semantic memory (Brain/RAG)",
            "stores": "Chunked project knowledge indexed by meaning",
            "query": "Natural language search — brain_search / brain_recall",
            "update": "After major code changes or architectural decisions"
          },
          "system_2_status": {
            "type": "Sequential memory (STATUS.txt)",
            "stores": "Current phase, last action, next step, blockers",
            "query": "Direct file read",
            "update": "After EVERY subtask completion"
          },
          "system_3_tasklist": {
            "type": "Structural memory (Task List)",
            "stores": "All tasks, dependencies, completion states, progress",
            "query": "TaskList / TaskGet",
            "update": "As tasks progress"
          },
          "checkpoint_protocol": {
            "when": "After every subtask, before session breaks, when context > 70%",
            "save_to": "All 3 systems",
            "contents": [
              "What was completed",
              "Files modified (with line counts)",
              "Key decisions made",
              "Current system state",
              "What comes next",
              "Blockers / open questions"
            ]
          },
          "session_recovery": {
            "mandatory_steps": [
              "1. Read HARDCODE RULES",
              "2. Read STATUS.txt — project state",
              "3. Read Task List — progress and next task",
              "4. Query Brain for current phase context",
              "5. Read SPECIFIC files needed for next subtask",
              "6. Produce Build Anchor Check",
              "7. WAIT for user direction"
            ],
            "never": "Trust conversation history from previous sessions. Re-read from FILES."
          }
        },

        "failure_recovery": {
          "on_anchor_lost": [
            "STOP immediately — do not continue writing code",
            "State: Build anchor lost. Initiating recovery.",
            "Re-read STATUS.txt",
            "Re-read Task List",
            "Re-read last Change Manifest or Breadcrumb",
            "Re-read target files from disk",
            "Produce NEW Build Anchor Check",
            "Continue from last verified point"
          ],
          "on_hallucination_detected": [
            "STOP immediately",
            "State: Potential hallucination. Verifying against codebase.",
            "Search codebase for the function/type in question",
            "If it does not exist: discard that code block entirely",
            "Re-anchor from actual codebase files",
            "Rewrite from verified reality"
          ],
          "on_user_says_lost": [
            "STOP immediately",
            "Apologize briefly (1 sentence max)",
            "Execute full Session Recovery Protocol",
            "Present: Here is where I think we are: [summary from files]",
            "WAIT for user to confirm or correct"
          ]
        },

        "rollback_protocol": {
          "description": "Formal undo procedure when a change passes all gates but still breaks something",
          "immediate_rollback": [
            "STOP all changes in progress",
            "Document WHAT broke and WHICH change caused it",
            "Revert changed files to last known good state"
          ],
          "method": "Full SPFsmartGATE folder zip/unzip — retains all settings, DBs, and files",
          "future_mcp_tools": {
            "spf_backup": "Zip entire SPFsmartGATE folder to timestamped archive",
            "spf_restore": "Restore from archive — full state recovery",
            "status": "PLANNED — to be built as Rust MCP tools"
          },
          "before_risky_changes": [
            "Recommend user run backup before Block execution",
            "Note current state in STATUS.txt",
            "Save checkpoint to Memory Triad"
          ]
        },

        "output_limits": {
          "quality_threshold": "500 lines per response (high coherence zone)",
          "hard_max": "4000 lines per response",
          "max_files_per_subtask": 7,
          "max_reasoning_chain": "10 dependent logical steps",
          "max_simultaneous_subsystems": 7
        },

        "capacity_reference": {
          "context_window_total": "200,000 tokens",
          "effective_working_memory": "40,000 tokens",
          "memory_decay": "15-25% loss per 50K tokens of new context",
          "single_pass_verification": "75% error detection rate",
          "note": "These are observed values for Claude Opus 4.5 — recalibrate if model changes"
        }
      }
    },

    "project_folders": {
      "work": "SPFsmartGATE/LIVE/PROJECTS/PROJECTS/",
      "original": "SPFsmartGATE/src/ (VIEW ONLY)",
      "status_file": "SPFsmartGATE/LIVE/PROJECTS/PROJECTS/STATUS.txt",
      "rules_file": "SPFsmartGATE/LIVE/PROJECTS/PROJECTS/HARDCODE-RULES.md",
      "sandbox": {
        "projects": "SPFsmartGATE/LIVE/PROJECTS/PROJECTS/*",
        "tmp": "SPFsmartGATE/LIVE/TMP/TMP/*",
        "policy": "ALL subdirectories within PROJECTS/PROJECTS/ and TMP/TMP/ are writable sandbox. Everything outside is READ ONLY or BLOCKED."
      }
    },

    "blocked_paths": {
      "description": "These files are READ ONLY — never write, edit, or overwrite",
      "claude_configs": [
        "~/.claude.json",
        "~/.claude/",
        "~/.claude/settings.json",
        "~/SPFsmartGATE/.claude.json",
        "~/SPFsmartGATE/CLAUDE.md",
        "~/SPFsmartGATE/LIVE/CLAUDE.md"
      ],
      "spf_core": [
        "~/SPFsmartGATE/src/",
        "~/SPFsmartGATE/Cargo.toml",
        "~/SPFsmartGATE/Cargo.lock"
      ],
      "lmdb_databases": [
        "~/SPFsmartGATE/LIVE/CONFIG/CONFIG.DB/",
        "~/SPFsmartGATE/LIVE/SESSION/SESSION.DB/",
        "~/SPFsmartGATE/LIVE/PROJECTS/PROJECTS.DB/",
        "~/SPFsmartGATE/LIVE/TMP/TMP.DB/",
        "~/SPFsmartGATE/LIVE/LMDB5/LMDB5.DB/",
        "~/SPFsmartGATE/LIVE/SPF_FS/SPF_FS.DB/"
      ],
      "system": [
        "/tmp",
        "/etc",
        "/usr",
        "/system"
      ],
      "policy": "Writes to any blocked path are REJECTED. Read access is system-wide open for development."
    }
  }


