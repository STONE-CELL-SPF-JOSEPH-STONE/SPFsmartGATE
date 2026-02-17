#!/bin/bash
# SPF Pre-Write Hook - BLOCKS Native Write
# Copyright 2026 Joseph Stone - All Rights Reserved
#
# Native Write tool is BLOCKED. Use spf_write instead.

echo "BLOCKED: Native Write disabled. Use spf_write with approved=true"
exit 1
