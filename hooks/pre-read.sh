#!/bin/bash
# SPF Pre-Read Hook - BLOCKS Native Read
# Copyright 2026 Joseph Stone - All Rights Reserved
#
# Native Read tool is BLOCKED. Use spf_read instead.

echo "BLOCKED: Native Read disabled. Use spf_read with approved=true"
exit 1
